// Letters written on the Mail page and sent: a new one, a reply (to the
// sender or to all), one passed on (its attachments with it, those still
// wanted). Made into MIME by mail-builder, handed to the account's outgoing
// server (smtp.rs); then, on its incoming one, a copy kept in the Sent
// folder (not where the server keeps one itself: Gmail) and the letter
// replied to marked answered. An account set up before the pet sent mail
// gets the outgoing server the lookup finds, kept once a letter went
// through it. Only the page sends, when Send is pressed; an agent never
// does.
use std::sync::Arc;

use base64::Engine;
use io_imap::types::flag::Flag;
use mail_builder::headers::address::Address;
use mail_builder::MessageBuilder;
use mail_parser::MimeHeaders;
use serde_json::{json, Value};
use tauri::AppHandle;

use super::imap::{session, Fail};
use super::{account, accounts, discover, keep_accounts, letters, password_of, smtp, split, Account, Server, CONNECT, EDITING};
use crate::{now_ms, shared, Shared};

// The most a letter may take with it (attachments, before encoding), how
// many it may go to, how long its text and subject may be.
const MOST_FILES: usize = 25 << 20;
const MOST_PEOPLE: usize = 100;
const MOST_TEXT: usize = 1 << 20;
const MOST_SUBJECT: usize = 500;

// Someone a letter goes to: a name (may be empty), an address.
#[derive(Clone, Debug, PartialEq)]
struct Person {
    name: String,
    address: String,
}

// The people in a line as one types it: "Name <a@b.c>, d@e.f; \"Li, Si\"
// <g@h.i>"; Err with the piece that is no address.
fn people_in(line: &str) -> Result<Vec<Person>, String> {
    let mut pieces = Vec::new();
    let (mut piece, mut quoted, mut angled) = (String::new(), false, false);
    for c in line.chars() {
        match c {
            '"' if !angled => quoted = !quoted,
            '<' if !quoted => angled = true,
            '>' if !quoted => angled = false,
            ',' | ';' | '\n' | '\r' if !quoted && !angled => {
                pieces.push(std::mem::take(&mut piece));
                continue;
            }
            _ => {}
        }
        piece.push(c);
    }
    pieces.push(piece);
    pieces.iter().map(|p| p.trim()).filter(|p| !p.is_empty()).map(|p| person(p).ok_or_else(|| p.to_string())).collect()
}

// One: "Name <a@b.c>", or "a@b.c".
fn person(piece: &str) -> Option<Person> {
    let (name, address) = match piece.rfind('<') {
        Some(at) if piece.ends_with('>') => (piece[..at].trim().trim_matches('"').trim(), piece[at + 1..piece.len() - 1].trim()),
        _ => ("", piece),
    };
    let ok = address.len() <= 254 && split(address).is_some() && !address.contains(['<', '>', '"', ',', ';', '(', ')', '[', ']', '\\']);
    ok.then(|| Person { name: name.chars().filter(|c| !c.is_control()).take(100).collect(), address: address.to_string() })
}

// A Message-ID as one is kept (no brackets): no spaces, brackets, or more.
fn is_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 500 && id.contains('@') && !id.chars().any(|c| c.is_whitespace() || c.is_control() || matches!(c, '<' | '>'))
}

// A letter as the page wrote it.
struct Draft {
    account: Account,
    to: Vec<Person>,
    cc: Vec<Person>,
    bcc: Vec<Person>,
    subject: String,
    text: String,
    // A reply: the letter's Message-ID and its References, and which it is
    // (account, UID) to mark answered.
    reply: Option<(String, Vec<String>)>,
    answered: Option<(String, u32)>,
    // Passed on: whose, which, and the attachments of it still wanted.
    forward: Option<(Account, u32, Vec<usize>)>,
    // Name, type, contents.
    files: Vec<(String, String, Vec<u8>)>,
}

fn refused(kind: &str, text: &str) -> Value {
    json!({ "ok": false, "error": { "kind": kind, "text": text } })
}

// A file's name as it goes: no folders, nothing Windows or a header
// cannot have.
fn file_name(name: &str) -> String {
    let name: String = name.rsplit(['/', '\\']).next().unwrap_or("").chars().filter(|c| !c.is_control() && !matches!(c, '"' | '<' | '>' | '|' | '?' | '*' | ':')).take(200).collect();
    let name = name.trim().trim_matches('.').to_string();
    if name.is_empty() { "attachment".into() } else { name }
}

// A type as "text/plain" is, else the one for any bytes.
fn file_type(kind: &str) -> String {
    let ok = kind.split_once('/').is_some_and(|(a, b)| [a, b].iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c))));
    if ok { kind.to_ascii_lowercase() } else { "application/octet-stream".into() }
}

// The page's letter, checked: Err as it says what is wrong.
fn draft_of(sh: &Shared, v: &Value) -> Result<Draft, Value> {
    let account = account(sh, v["account"].as_str().unwrap_or("")).ok_or_else(|| refused("gone", ""))?;
    let line = |key: &str| people_in(v[key].as_str().unwrap_or("")).map_err(|piece| refused("address", &piece.chars().take(80).collect::<String>()));
    let (to, cc, bcc) = (line("to")?, line("cc")?, line("bcc")?);
    let count = to.len() + cc.len() + bcc.len();
    if count == 0 {
        return Err(refused("noOne", ""));
    }
    if count > MOST_PEOPLE {
        return Err(refused("tooMany", ""));
    }
    let subject: String = v["subject"].as_str().unwrap_or("").chars().map(|c| if c.is_control() { ' ' } else { c }).take(MOST_SUBJECT).collect();
    let text = v["text"].as_str().unwrap_or("").to_string();
    if text.len() > MOST_TEXT {
        return Err(refused("tooLong", ""));
    }
    let r = &v["reply"];
    let reply = r["messageId"].as_str().filter(|id| is_id(id)).map(|id| {
        let refs: Vec<String> = r["references"].as_array().into_iter().flatten().filter_map(Value::as_str).filter(|x| is_id(x) && *x != id).map(String::from).collect();
        (id.to_string(), refs[refs.len().saturating_sub(20)..].to_vec())
    });
    let answered = r["account"].as_str().zip(r["uid"].as_u64()).map(|(a, uid)| (a.to_string(), uid as u32));
    let f = &v["forward"];
    let forward = match (f["account"].as_str(), f["uid"].as_u64()) {
        (Some(a), Some(uid)) => {
            let whose = super::account(sh, a).ok_or_else(|| refused("gone", ""))?;
            let keep = f["keep"].as_array().into_iter().flatten().filter_map(Value::as_u64).map(|i| i as usize).collect();
            Some((whose, uid as u32, keep))
        }
        _ => None,
    };
    let mut files = Vec::new();
    let mut size = 0;
    for file in v["files"].as_array().into_iter().flatten() {
        let data = base64::engine::general_purpose::STANDARD.decode(file["data"].as_str().unwrap_or("")).map_err(|_| refused("file", file["name"].as_str().unwrap_or("")))?;
        size += data.len();
        if size > MOST_FILES {
            return Err(refused("tooBig", ""));
        }
        files.push((file_name(file["name"].as_str().unwrap_or("")), file_type(file["type"].as_str().unwrap_or("")), data));
    }
    Ok(Draft { account, to, cc, bcc, subject: subject.trim().to_string(), text, reply, answered, forward, files })
}

// A Message-ID of our own, at the sender's domain (not the machine's name).
fn message_id(address: &str) -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u64(now_ms());
    h.write_u32(std::process::id());
    let domain = split(address).map(|(_, d)| d).unwrap_or_else(|| "wakuwaku.invalid".into());
    format!("{:016x}.{}@{domain}", h.finish(), now_ms())
}

fn list(people: &[Person]) -> Address<'_> {
    Address::new_list(people.iter().map(|p| Address::new_address((!p.name.is_empty()).then_some(p.name.as_str()), p.address.as_str())).collect())
}

// The letter whole: who from (with the account's name), to, cc, (for the
// copy kept) bcc, the subject, what it answers, its text and files.
fn message(d: &Draft, id: &str, date: u64, with_bcc: bool) -> Vec<u8> {
    let from = Address::new_address((!d.account.name.is_empty()).then_some(d.account.name.as_str()), d.account.address.as_str());
    let mut b = MessageBuilder::new().from(from).subject(d.subject.as_str()).message_id(id).date(date).text_body(d.text.as_str());
    if !d.to.is_empty() {
        b = b.to(list(&d.to));
    }
    if !d.cc.is_empty() {
        b = b.cc(list(&d.cc));
    }
    if with_bcc && !d.bcc.is_empty() {
        b = b.bcc(list(&d.bcc));
    }
    if let Some((answers, refs)) = &d.reply {
        let mut all: Vec<&str> = refs.iter().map(String::as_str).collect();
        all.push(answers);
        b = b.in_reply_to(answers.as_str()).references(all);
    }
    for (name, kind, data) in &d.files {
        b = b.attachment(kind.as_str(), name.as_str(), data.as_slice());
    }
    b.write_to_vec().unwrap_or_default()
}

// Everyone it goes to, each once.
fn everyone(d: &Draft) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for p in d.to.iter().chain(&d.cc).chain(&d.bcc) {
        if !out.iter().any(|a| a.eq_ignore_ascii_case(&p.address)) {
            out.push(p.address.clone());
        }
    }
    out
}

// Gmail keeps what goes out through its server in Sent itself.
fn keeps_sent_itself(host: &str) -> bool {
    host == "imap.gmail.com" || host == "imap.googlemail.com"
}

// The outgoing server found for an account that had none, kept with it.
fn keep_outgoing(sh: &Arc<Shared>, id: &str, server: Server) {
    let _editing = EDITING.lock().unwrap();
    let mut all = accounts(sh);
    if let Some(a) = all.iter_mut().find(|a| a.id == id && a.smtp.is_none()) {
        a.smtp = Some(server);
        keep_accounts(sh, &all);
    }
}

// After it went, on the incoming server: a copy in Sent (kept, or the
// server's own, or no Sent folder), and the letter it answers marked
// answered where it is this account's. What came of the copy.
fn afterwards(account: &Account, password: &str, copy: &[u8], answered: Option<u32>) -> Result<&'static str, Fail> {
    let mut s = session(&account.server, password, CONNECT)?;
    let kept = if keeps_sent_itself(&account.server.host) {
        "server"
    } else {
        match s.sent_box()? {
            Some(sent) => {
                s.append(sent, copy)?;
                "kept"
            }
            None => "none",
        }
    };
    if let Some(uid) = answered {
        s.inbox(true)?;
        s.set_flag(uid, Flag::Answered, true)?;
    }
    s.logout();
    Ok(kept)
}

// A letter answered on another account's server.
fn mark_answered(account: &Account, uid: u32) -> Result<(), Fail> {
    let mut s = session(&account.server, &password_of(account)?, CONNECT)?;
    s.inbox(true)?;
    s.set_flag(uid, Flag::Answered, true)?;
    s.logout();
    Ok(())
}

fn send(sh: &Arc<Shared>, mut d: Draft) -> Value {
    let password = match password_of(&d.account) {
        Ok(p) => p,
        Err(fail) => return json!({ "ok": false, "error": fail.json() }),
    };
    // A letter passed on takes its attachments still wanted.
    if let Some((whose, uid, keep)) = d.forward.take() {
        if !keep.is_empty() {
            let raw = match letters::fetch(&whose, uid, false) {
                Ok(Some((raw, _))) => raw,
                Ok(None) => return refused("noLetter", ""),
                Err(fail) => return json!({ "ok": false, "error": fail.json() }),
            };
            let Some(m) = letters::parse(&raw) else { return refused("noLetter", "") };
            for (_, a) in m.attachments().enumerate().filter(|(i, _)| keep.contains(i)) {
                let kind = a.content_type().map(|t| format!("{}/{}", t.ctype(), t.subtype().unwrap_or("octet-stream"))).unwrap_or_default();
                d.files.push((file_name(a.attachment_name().unwrap_or("")), file_type(&kind), a.contents().to_vec()));
            }
        }
    }
    if d.files.iter().map(|f| f.2.len()).sum::<usize>() > MOST_FILES {
        return refused("tooBig", "");
    }
    // Its outgoing server; for an account from before, the one found.
    let (server, found) = match d.account.smtp.clone() {
        Some(s) => (s, false),
        None => match discover::outgoing_for(&d.account.address) {
            Some(s) => (Server { username: d.account.server.username.clone(), ..s }, true),
            None => return refused("noSmtp", ""),
        },
    };
    let id = message_id(&d.account.address);
    let date = now_ms() / 1000;
    let to = everyone(&d);
    let went = (|| -> Result<(), Fail> {
        let mut s = smtp::session(&server, &password, CONNECT)?;
        s.send(&d.account.address, &to, &message(&d, &id, date, false))?;
        s.quit();
        Ok(())
    })();
    if let Err(fail) = went {
        sh.log(&format!("mail {}: not sent: {:?}", d.account.id, fail));
        return json!({ "ok": false, "out": true, "error": fail.json() });
    }
    sh.log(&format!("mail {}: sent to {} ({} files)", d.account.id, to.len(), d.files.len()));
    if found {
        keep_outgoing(sh, &d.account.id, server);
    }
    let (own, other) = match d.answered.take() {
        Some((a, uid)) if a == d.account.id => (Some(uid), None),
        Some((a, uid)) => (None, account(sh, &a).map(|a| (a, uid))),
        None => (None, None),
    };
    let kept = afterwards(&d.account, &password, &message(&d, &id, date, true), own);
    if let Some((a, uid)) = other {
        if let Err(fail) = mark_answered(&a, uid) {
            sh.log(&format!("mail {}: not marked answered: {:?}", a.id, fail));
        }
    }
    match kept {
        Ok(kept) => json!({ "ok": true, "kept": kept }),
        Err(fail) => {
            sh.log(&format!("mail {}: no copy in Sent: {:?}", d.account.id, fail));
            json!({ "ok": true, "kept": "failed", "error": fail.json() })
        }
    }
}

// A letter sent, as the page wrote it: { account, to, cc, bcc (each a line
// of people), subject, text, reply: { account, uid, messageId, references },
// forward: { account, uid, keep: [attachment numbers] }, files: [{ name,
// type, data (base64) }] }.
#[tauri::command]
pub async fn mail_send(app: AppHandle, letter: Value) -> Value {
    let sh = shared(&app);
    let draft = match draft_of(&sh, &letter) {
        Ok(d) => d,
        Err(why) => return why,
    };
    tauri::async_runtime::spawn_blocking(move || send(&sh, draft)).await.unwrap_or_else(|e| json!({ "ok": false, "error": Fail::Other(e.to_string()).json() }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mail::{Auth, Security};

    #[test]
    fn people_are_read_as_one_types_them() {
        let got = people_in("Zhang San <zs@vendor.example>, li@example.test; \"Wang, Wu\" <ww@example.test>,, ").unwrap();
        let names: Vec<(&str, &str)> = got.iter().map(|p| (p.name.as_str(), p.address.as_str())).collect();
        assert_eq!(names, [("Zhang San", "zs@vendor.example"), ("", "li@example.test"), ("Wang, Wu", "ww@example.test")]);
        assert_eq!(people_in("王总 <boss@例子.test>"), Err("王总 <boss@例子.test>".into()));
        assert_eq!(people_in("zs@vendor.example li@example.test"), Err("zs@vendor.example li@example.test".into()));
        assert_eq!(people_in("no-at-sign"), Err("no-at-sign".into()));
        assert_eq!(people_in("  ").unwrap(), []);
    }

    fn draft() -> Draft {
        let server = Server { host: "imap.example.test".into(), port: 993, security: Security::Ssl, username: "me".into(), auth: Auth::Auto };
        let account = Account { id: "a1".into(), address: "me@example.test".into(), name: "周 Me".into(), server, smtp: None, on: true };
        Draft {
            account,
            to: people_in("Zhang San <zs@vendor.example>").unwrap(),
            cc: people_in("li@example.test").unwrap(),
            bcc: people_in("me2@example.test, ZS@vendor.example").unwrap(),
            subject: "Re: 合同第 8 条".into(),
            text: "好的，周五前回复。\n\n> 原信\n.end".into(),
            reply: Some(("c4@vendor.example".into(), vec!["c1@vendor.example".into()])),
            answered: Some(("a1".into(), 4)),
            forward: None,
            files: vec![("notes.txt".into(), "text/plain".into(), b"abc".to_vec())],
        }
    }

    #[test]
    fn a_letter_is_made_as_written() {
        let d = draft();
        let raw = message(&d, "x1@example.test", 1_791_512_100, false);
        let m = letters::parse(&raw).unwrap();
        assert_eq!(m.subject(), Some("Re: 合同第 8 条"));
        assert_eq!(m.from().unwrap().first().unwrap().name(), Some("周 Me"));
        assert_eq!(m.to().unwrap().first().unwrap().address(), Some("zs@vendor.example"));
        assert_eq!(m.message_id(), Some("x1@example.test"));
        assert_eq!(m.in_reply_to().as_text(), Some("c4@vendor.example"));
        assert_eq!(m.references().as_text_list().unwrap().len(), 2);
        assert_eq!(letters::text(&m), "好的，周五前回复。\n\n> 原信\n.end");
        assert_eq!(m.attachments().next().unwrap().attachment_name(), Some("notes.txt"));
        // Bcc only in the copy kept.
        assert!(m.bcc().is_none() && raw.is_ascii());
        assert_eq!(letters::parse(&message(&d, "x1@example.test", 0, true)).unwrap().bcc().unwrap().iter().count(), 2);
        assert_eq!(everyone(&d), ["zs@vendor.example", "li@example.test", "me2@example.test"]);
    }

    #[test]
    fn what_goes_out_is_tame() {
        assert_eq!(file_name("C:\\Users\\x\\合同 v2.pdf"), "合同 v2.pdf");
        assert_eq!(file_name("../\"a\".txt"), "a.txt");
        assert_eq!(file_name(""), "attachment");
        assert_eq!(file_type("Application/PDF"), "application/pdf");
        assert_eq!(file_type("text/html\r\nX: y"), "application/octet-stream");
        assert!(is_id("c4@vendor.example") && !is_id("<c4@vendor.example>") && !is_id("a b@c") && !is_id("nope"));
        assert!(message_id("me@Example.test").ends_with("@example.test"));
    }
}
