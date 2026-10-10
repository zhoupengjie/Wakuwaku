// The Mail page's inbox: the newest letters by their headers (one account's,
// or every account's as one; all, the unread or the starred), stars put on
// and taken off, and one letter read whole. Opening a letter on the page
// marks it read, as any mail program does; listing does not, nor does
// handing one to an agent. Each asks over a connection of its own, signed
// in and gone again.
use std::borrow::Cow;

use io_imap::types::flag::Flag;
use io_imap::types::search::SearchKey;
use mail_parser::{Address, Message, MessageParser, MimeHeaders};
use serde_json::{json, Value};
use tauri::AppHandle;

use super::imap::{session, Fail, Head};
use super::{account, accounts, agent, password_of, Account, CONNECT};
use crate::shared;

// The header fields a list row is made of.
const FIELDS: [&str; 5] = ["FROM", "SUBJECT", "DATE", "MESSAGE-ID", "CONTENT-TYPE"];
// The most letters asked for at once, and the most of a letter's text sent
// to the page.
const MOST: u32 = 200;
const MOST_TEXT: usize = 100_000;

pub fn parse(raw: &[u8]) -> Option<Message<'_>> {
    MessageParser::default().parse(raw)
}

// The people in a header, as { name, address }.
pub fn people(list: Option<&Address>) -> Vec<Value> {
    list.map(|a| a.iter().map(|p| json!({ "name": p.name().unwrap_or("").trim(), "address": p.address().unwrap_or("").trim() })).collect()).unwrap_or_default()
}

// When it was sent (ms), else when the server got it.
fn when(m: Option<&Message>, received: Option<i64>) -> Option<i64> {
    m.and_then(|m| m.date()).map(|d| d.to_timestamp()).or(received).map(|s| s * 1000)
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

// A letter's attachments: name, size, type.
pub fn attachments(m: &Message) -> Vec<Value> {
    m.attachments()
        .map(|a| {
            let kind = a.content_type().map(|t| format!("{}/{}", t.ctype(), t.subtype().unwrap_or(""))).unwrap_or_default();
            json!({ "name": a.attachment_name().unwrap_or(""), "size": a.contents().len(), "type": kind })
        })
        .collect()
}

// A row of the list, with whose it is.
fn row(account: &str, h: &Head) -> Value {
    let m = parse(&h.header);
    let from = m.as_ref().map(|m| people(m.from())).unwrap_or_default();
    let first = from.first();
    let mixed = m.as_ref().and_then(|m| m.root_part().content_type()).is_some_and(|t| t.ctype().eq_ignore_ascii_case("multipart") && t.subtype().is_some_and(|s| s.eq_ignore_ascii_case("mixed")));
    json!({
        "account": account,
        "uid": h.uid,
        "flagged": h.flagged,
        "answered": h.answered,
        "from": first.map_or("", |p| p["name"].as_str().filter(|n| !n.is_empty()).or(p["address"].as_str()).unwrap_or("")),
        "address": first.map_or("", |p| p["address"].as_str().unwrap_or("")),
        "subject": m.as_ref().and_then(|m| m.subject()).unwrap_or("").trim(),
        "date": when(m.as_ref(), h.received),
        "seen": h.seen,
        "size": h.size,
        "attached": mixed,
        "messageId": m.as_ref().and_then(|m| m.message_id()).unwrap_or(""),
    })
}

// Which letters the inbox shows: all, the unread, or the starred.
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

// The newest that came in, newest sent first (they do not always come in order).
fn newest_first(letters: &mut [Value]) {
    letters.sort_by_key(|l| std::cmp::Reverse(l["date"].as_i64().unwrap_or(0)));
}

// The newest `count` letters the filter lets through, newest first, and how
// many it lets through. All: the last by position. Unread or starred: the
// server finds them (SEARCH), the newest of them by UID.
fn list(account: &Account, count: u32, filter: Filter) -> Result<(u32, Vec<Value>), Fail> {
    let count = count.clamp(1, MOST);
    let mut s = session(&account.server, &password_of(account)?, CONNECT)?;
    let exists = s.inbox(false)?;
    let (total, heads) = match filter {
        Filter::All if exists == 0 => (0, Vec::new()),
        Filter::All => (exists, s.heads(&format!("{}:{exists}", exists.saturating_sub(count) + 1), false, &FIELDS)?),
        Filter::Unseen | Filter::Flagged => {
            let mut uids = s.search(if filter == Filter::Unseen { SearchKey::Unseen } else { SearchKey::Flagged })?;
            uids.sort_unstable();
            let total = uids.len() as u32;
            let newest = &uids[uids.len().saturating_sub(count as usize)..];
            let set = newest.iter().map(u32::to_string).collect::<Vec<_>>().join(",");
            (total, if newest.is_empty() { Vec::new() } else { s.heads(&set, true, &FIELDS)? })
        }
    };
    s.logout();
    let mut letters: Vec<Value> = heads.iter().map(|h| row(&account.id, h)).collect();
    newest_first(&mut letters);
    Ok((total, letters))
}

// Every account's newest, side by side (a thread each), as one inbox: the
// newest `count` of them all, how many in all, and the accounts that could
// not be asked, with why.
fn list_all(all: Vec<Account>, count: u32, filter: Filter) -> Value {
    let asked: Vec<_> = all.into_iter().map(|a| std::thread::spawn(move || (a.id.clone(), list(&a, count, filter)))).collect();
    let (mut total, mut letters, mut errors) = (0, Vec::new(), Vec::new());
    for got in asked {
        match got.join() {
            Ok((_, Ok((n, rows)))) => {
                total += n;
                letters.extend(rows);
            }
            Ok((id, Err(fail))) => errors.push(json!({ "account": id, "error": fail.json() })),
            Err(_) => {}
        }
    }
    newest_first(&mut letters);
    letters.truncate(count.clamp(1, MOST) as usize);
    json!({ "ok": true, "total": total, "letters": letters, "errors": errors })
}

// A letter as it came, by UID, and whether it is starred; marked read when `mark`.
pub fn fetch(account: &Account, uid: u32, mark: bool) -> Result<Option<(Vec<u8>, bool)>, Fail> {
    let mut s = session(&account.server, &password_of(account)?, CONNECT)?;
    s.inbox(mark)?;
    let raw = s.letter(uid)?;
    if mark && raw.is_some() {
        s.mark_seen(uid)?;
    }
    s.logout();
    Ok(raw)
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
        "date": when(Some(m), None),
        "messageId": m.message_id().unwrap_or(""),
        "text": cut,
        "cut": cut.len() < text.len(),
        "attachments": attachments(m),
    })
}

fn failed(fail: Fail) -> Value {
    json!({ "ok": false, "error": fail.json() })
}

// An inbox's newest letters: one account's, or ("*") every account's as one.
#[tauri::command]
pub async fn mail_letters(app: AppHandle, id: String, count: u32, filter: String) -> Value {
    let sh = shared(&app);
    let filter = Filter::parse(&filter);
    if id == "*" {
        let all = accounts(&sh);
        return tauri::async_runtime::spawn_blocking(move || list_all(all, count, filter)).await.unwrap_or_else(|e| failed(Fail::Other(e.to_string())));
    }
    let Some(account) = account(&sh, &id) else { return json!({ "ok": false, "error": { "kind": "gone", "text": "" } }) };
    let got = tauri::async_runtime::spawn_blocking(move || list(&account, count, filter)).await.unwrap_or_else(|e| Err(Fail::Other(e.to_string())));
    match got {
        Ok((total, letters)) => json!({ "ok": true, "total": total, "letters": letters, "errors": [] }),
        Err(fail) => failed(fail),
    }
}

// A letter starred, or not any more.
#[tauri::command]
pub async fn mail_flag(app: AppHandle, id: String, uid: u32, on: bool) -> Value {
    let Some(account) = account(&shared(&app), &id) else { return json!({ "ok": false, "error": { "kind": "gone", "text": "" } }) };
    let got = tauri::async_runtime::spawn_blocking(move || -> Result<(), Fail> {
        let mut s = session(&account.server, &password_of(&account)?, CONNECT)?;
        s.inbox(true)?;
        s.set_flag(uid, Flag::Flagged, on)?;
        s.logout();
        Ok(())
    })
    .await
    .unwrap_or_else(|e| Err(Fail::Other(e.to_string())));
    match got {
        Ok(()) => json!({ "ok": true }),
        Err(fail) => failed(fail),
    }
}

// A letter opened on the page: read whole and marked read; with its talk
// with an agent, if there is one.
#[tauri::command]
pub async fn mail_letter(app: AppHandle, id: String, uid: u32) -> Value {
    let sh = shared(&app);
    let Some(account) = account(&sh, &id) else { return json!({ "ok": false, "error": { "kind": "gone", "text": "" } }) };
    let got = tauri::async_runtime::spawn_blocking(move || fetch(&account, uid, true)).await.unwrap_or_else(|e| Err(Fail::Other(e.to_string())));
    let (raw, flagged) = match got {
        Ok(Some((raw, flagged))) => (raw, flagged),
        Ok(None) => return json!({ "ok": false, "error": { "kind": "noLetter", "text": "" } }),
        Err(fail) => return failed(fail),
    };
    let Some(m) = parse(&raw) else { return json!({ "ok": false, "error": { "kind": "other", "text": "unreadable" } }) };
    let mut letter = view(&m);
    letter["uid"] = json!(uid);
    let key = agent::key(&id, uid, m.message_id());
    letter["key"] = json!(key);
    letter["account"] = json!(id);
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
        let head = Head { uid: 4, seen: false, flagged: true, answered: true, size: 9, received: None, header: raw.to_vec() };
        let r = row("a1", &head);
        assert_eq!((r["from"].as_str(), r["attached"].as_bool(), r["answered"].as_bool()), (Some("Zhang San"), Some(true), Some(true)));
    }
}
