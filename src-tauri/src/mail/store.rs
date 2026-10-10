// The mail kept on this computer, as Thunderbird keeps it: for each
// account, its inbox and its Sent folder, every letter's headers and flags
// (an index, <data>/mail-store/<account>/<folder>.json) and, as they come
// down, each whole letter (<folder>/<uid>.eml). The sync (sync.rs) keeps it
// as the server has it; the Mail page reads it (letters.rs, threads.rs)
// without asking the server. A folder whose UIDVALIDITY changed starts
// again, and so does an account whose server or user changed.
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use super::letters::{parse, text};
use super::{Account, Server};
use crate::Shared;

// A flag changed here is the one shown until the server has had time to
// take it (a sync in between may still see the old one).
const PENDING_FOR: Duration = Duration::from_secs(60);
// How much of a letter's text the list shows.
const SNIPPET: usize = 140;

// Which folder of an account.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Folder {
    Inbox,
    Sent,
}

impl Folder {
    pub fn parse(s: &str) -> Folder {
        if s == "sent" {
            Folder::Sent
        } else {
            Folder::Inbox
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Folder::Inbox => "inbox",
            Folder::Sent => "sent",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Person {
    pub name: String,
    pub address: String,
}

// A letter as kept: what the server knows of it (flags, size, when it
// came), its headers, and whether the whole of it is here.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Entry {
    pub uid: u32,
    pub seen: bool,
    pub flagged: bool,
    pub answered: bool,
    pub size: u32,
    // When the server got it (seconds).
    pub received: Option<i64>,
    // When it was sent (ms), else when the server got it.
    pub date: Option<i64>,
    pub from: Person,
    pub to: Vec<Person>,
    pub cc: Vec<Person>,
    pub subject: String,
    // Message-ID, In-Reply-To, References (no brackets): its thread.
    pub id: String,
    pub parent: String,
    pub refs: Vec<String>,
    pub attached: bool,
    // The whole letter is kept (<folder>/<uid>.eml), and the start of its text.
    pub body: bool,
    pub snippet: String,
}

fn person(a: Option<&mail_parser::Addr>) -> Person {
    a.map(|a| Person { name: a.name().unwrap_or("").trim().into(), address: a.address().unwrap_or("").trim().into() }).unwrap_or_default()
}

fn people(list: Option<&mail_parser::Address>) -> Vec<Person> {
    list.map(|l| l.iter().map(|a| person(Some(a))).collect()).unwrap_or_default()
}

fn ids(v: &mail_parser::HeaderValue) -> Vec<String> {
    v.as_text_list().map(|l| l.iter().map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()).unwrap_or_default()
}

impl Entry {
    // A letter's entry from its header fields and what the server knows.
    pub fn from_head(uid: u32, flags: (bool, bool, bool), size: u32, received: Option<i64>, header: &[u8]) -> Entry {
        let mut e = Entry { uid, seen: flags.0, flagged: flags.1, answered: flags.2, size, received, ..Default::default() };
        e.date = received.map(|s| s * 1000);
        if let Some(m) = parse(header) {
            e.read_headers(&m);
        }
        e
    }

    fn read_headers(&mut self, m: &mail_parser::Message) {
        self.from = person(m.from().and_then(|a| a.first()));
        self.to = people(m.to());
        self.cc = people(m.cc());
        self.subject = m.subject().unwrap_or("").trim().into();
        self.id = m.message_id().unwrap_or("").trim().into();
        self.parent = ids(m.in_reply_to()).pop().unwrap_or_default();
        self.refs = ids(m.references());
        if let Some(d) = m.date() {
            self.date = Some(d.to_timestamp() * 1000);
        }
        use mail_parser::MimeHeaders;
        self.attached = m.root_part().content_type().is_some_and(|t| t.ctype().eq_ignore_ascii_case("multipart") && t.subtype().is_some_and(|s| s.eq_ignore_ascii_case("mixed")));
    }

    // The letter whole is here: the start of its text, and its attachments
    // as they are (not as its type guessed).
    fn read_body(&mut self, raw: &[u8]) {
        let Some(m) = parse(raw) else { return };
        if self.id.is_empty() {
            self.read_headers(&m);
        }
        let words = text(&m).split_whitespace().collect::<Vec<_>>().join(" ");
        self.snippet = words.chars().take(SNIPPET).collect();
        self.attached = m.attachments().next().is_some();
        self.body = true;
    }
}

// A folder as kept: its name on the server (the inbox is INBOX; the Sent
// folder's is found), its UIDVALIDITY, its letters by UID.
#[derive(Debug, Default)]
pub struct Kept {
    pub mailbox: String,
    pub validity: u32,
    pub letters: BTreeMap<u32, Entry>,
    dirty: bool,
}

// As written down.
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct File {
    server: String,
    mailbox: String,
    validity: u32,
    letters: Vec<Entry>,
}

#[derive(Debug)]
pub struct Store {
    dir: PathBuf,
    // Whose: the server and user the letters came from.
    server: String,
    pub inbox: Kept,
    pub sent: Kept,
    // Flags changed here and when.
    pending: HashMap<(Folder, u32), Instant>,
    // The first sync of each folder done (the list is whole).
    pub synced: [bool; 2],
}

// Whose the letters are: another server, user or way, other letters.
fn server_key(s: &Server) -> String {
    format!("{}:{}:{}", s.host, s.port, s.username.to_lowercase())
}

// Bumped whenever any store changes: the page asks for the list again.
static REV: AtomicU64 = AtomicU64::new(1);

pub fn rev() -> u64 {
    REV.load(Ordering::SeqCst)
}

pub fn bump() {
    REV.fetch_add(1, Ordering::SeqCst);
}

impl Store {
    // An account's store as it was left, or a new one (another server's
    // letters gone).
    pub fn load(dir: PathBuf, server: &Server) -> Store {
        let key = server_key(server);
        let mut s = Store { dir, server: key.clone(), inbox: Kept { mailbox: "INBOX".into(), ..Default::default() }, sent: Kept::default(), pending: HashMap::new(), synced: [false; 2] };
        for folder in [Folder::Inbox, Folder::Sent] {
            let path = s.dir.join(format!("{}.json", folder.name()));
            let Some(file) = std::fs::read(&path).ok().and_then(|b| serde_json::from_slice::<File>(&b).ok()) else { continue };
            if file.server != key {
                // Another server's: gone, files and all.
                let _ = std::fs::remove_dir_all(&s.dir);
                s.inbox = Kept { mailbox: "INBOX".into(), ..Default::default() };
                s.sent = Kept::default();
                return s;
            }
            let kept = s.kept(folder);
            kept.mailbox = if folder == Folder::Inbox { "INBOX".into() } else { file.mailbox };
            kept.validity = file.validity;
            kept.letters = file.letters.into_iter().map(|e| (e.uid, e)).collect();
        }
        s
    }

    pub fn kept(&mut self, folder: Folder) -> &mut Kept {
        match folder {
            Folder::Inbox => &mut self.inbox,
            Folder::Sent => &mut self.sent,
        }
    }

    pub fn get(&self, folder: Folder) -> &Kept {
        match folder {
            Folder::Inbox => &self.inbox,
            Folder::Sent => &self.sent,
        }
    }

    pub fn body_path(&self, folder: Folder, uid: u32) -> PathBuf {
        self.dir.join(folder.name()).join(format!("{uid}.eml"))
    }

    // The folder begun again: another UIDVALIDITY (or another Sent folder).
    pub fn reset(&mut self, folder: Folder, mailbox: &str, validity: u32) {
        let _ = std::fs::remove_dir_all(self.dir.join(folder.name()));
        let kept = self.kept(folder);
        *kept = Kept { mailbox: mailbox.into(), validity, letters: BTreeMap::new(), dirty: true };
        bump();
    }

    // The flags the server has: kept, unless changed here a moment ago;
    // letters it no longer has, gone (their files too). The UIDs it has that
    // are not kept yet, newest first.
    pub fn take_flags(&mut self, folder: Folder, all: &[super::imap::Flags]) -> Vec<u32> {
        let now = Instant::now();
        self.pending.retain(|_, at| now.duration_since(*at) < PENDING_FOR);
        let pending: Vec<u32> = self.pending.keys().filter(|(f, _)| *f == folder).map(|(_, uid)| *uid).collect();
        let there: std::collections::HashSet<u32> = all.iter().map(|f| f.uid).collect();
        let gone: Vec<u32> = self.get(folder).letters.keys().filter(|uid| !there.contains(uid)).copied().collect();
        let mut changed = !gone.is_empty();
        for uid in &gone {
            let _ = std::fs::remove_file(self.body_path(folder, *uid));
        }
        let kept = self.kept(folder);
        for uid in gone {
            kept.letters.remove(&uid);
        }
        let mut new = Vec::new();
        for f in all {
            match kept.letters.get_mut(&f.uid) {
                Some(e) if !pending.contains(&f.uid) => {
                    if (e.seen, e.flagged, e.answered) != (f.seen, f.flagged, f.answered) {
                        (e.seen, e.flagged, e.answered) = (f.seen, f.flagged, f.answered);
                        changed = true;
                    }
                }
                Some(_) => {}
                None => new.push(f.uid),
            }
        }
        if changed {
            kept.dirty = true;
            bump();
        }
        new.sort_unstable_by(|a, b| b.cmp(a));
        new
    }

    pub fn add(&mut self, folder: Folder, e: Entry) {
        let kept = self.kept(folder);
        kept.letters.insert(e.uid, e);
        kept.dirty = true;
        bump();
    }

    // A flag changed here (the server told, or to be told).
    pub fn set_flag(&mut self, folder: Folder, uid: u32, what: &str, on: bool) {
        let kept = self.kept(folder);
        let Some(e) = kept.letters.get_mut(&uid) else { return };
        match what {
            "seen" => e.seen = on,
            "flagged" => e.flagged = on,
            "answered" => e.answered = on,
            _ => return,
        }
        kept.dirty = true;
        self.pending.insert((folder, uid), Instant::now());
        bump();
    }

    // A whole letter, kept beside its entry.
    pub fn keep_body(&mut self, folder: Folder, uid: u32, raw: &[u8]) {
        let path = self.body_path(folder, uid);
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if write_whole(&path, raw).is_err() {
            return;
        }
        let kept = self.kept(folder);
        if let Some(e) = kept.letters.get_mut(&uid) {
            e.read_body(raw);
            kept.dirty = true;
            bump();
        }
    }

    // A whole letter, if it is kept.
    pub fn body(&self, folder: Folder, uid: u32) -> Option<Vec<u8>> {
        self.get(folder).letters.get(&uid).filter(|e| e.body)?;
        std::fs::read(self.body_path(folder, uid)).ok()
    }

    // The letters still to come down, newest first, the inbox's before Sent's.
    pub fn missing(&self) -> Vec<(Folder, u32)> {
        let mut out = Vec::new();
        for folder in [Folder::Inbox, Folder::Sent] {
            out.extend(self.get(folder).letters.values().rev().filter(|e| !e.body).map(|e| (folder, e.uid)));
        }
        out
    }

    // How much is kept whole: letters, of how many, and bytes.
    pub fn offline(&self) -> (usize, usize, u64) {
        let all = self.inbox.letters.values().chain(self.sent.letters.values());
        all.fold((0, 0, 0), |(have, total, bytes), e| (have + e.body as usize, total + 1, bytes + if e.body { e.size as u64 } else { 0 }))
    }

    // What changed, written down (each folder's index as a whole, by way of
    // a file beside it, so a crash leaves the old one).
    pub fn save(&mut self) {
        for folder in [Folder::Inbox, Folder::Sent] {
            if !self.get(folder).dirty {
                continue;
            }
            let k = self.get(folder);
            let file = File { server: self.server.clone(), mailbox: k.mailbox.clone(), validity: k.validity, letters: k.letters.values().cloned().collect() };
            let _ = std::fs::create_dir_all(&self.dir);
            if let Ok(bytes) = serde_json::to_vec(&file) {
                if write_whole(&self.dir.join(format!("{}.json", folder.name())), &bytes).is_ok() {
                    self.kept(folder).dirty = false;
                }
            }
        }
    }
}

// A file written whole: to a file beside it, then put in its place.
fn write_whole(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("part");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

// Every account's store, loaded when first asked for.
// (By account: whose letters, and the store. A store's lock is never held
// while the mail runner's is taken.)
#[derive(Default)]
pub struct Stores {
    by_account: HashMap<String, (String, Arc<Mutex<Store>>)>,
}

// Under the data folder, wherever that is (a test copy's own).
fn dir_of(sh: &Shared, id: &str) -> PathBuf {
    sh.dir.join("mail-store").join(id)
}

// An account's store (its server's letters; another server's are gone).
pub fn of(sh: &Shared, account: &Account) -> Arc<Mutex<Store>> {
    let mut r = sh.mail.lock().unwrap();
    let key = server_key(&account.server);
    if let Some((k, s)) = r.stores.by_account.get(&account.id) {
        if *k == key {
            return s.clone();
        }
    }
    let s = Arc::new(Mutex::new(Store::load(dir_of(sh, &account.id), &account.server)));
    r.stores.by_account.insert(account.id.clone(), (key, s.clone()));
    s
}

// An account removed: its letters too.
pub fn forget(sh: &Shared, id: &str) {
    sh.mail.lock().unwrap().stores.by_account.remove(id);
    let _ = std::fs::remove_dir_all(dir_of(sh, id));
    bump();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mail::{imap::Flags, Auth, Security};

    fn server(user: &str) -> Server {
        Server { host: "imap.example.test".into(), port: 993, security: Security::Ssl, username: user.into(), auth: Auth::Auto }
    }

    const HEAD: &[u8] = b"From: \"Zhang San\" <zs@vendor.example>\r\nTo: me@example.test\r\nSubject: =?UTF-8?B?5ZCI5ZCM?=\r\nDate: Fri, 09 Oct 2026 10:15:00 +0800\r\nMessage-ID: <c4@vendor.example>\r\nIn-Reply-To: <c3@example.test>\r\nReferences: <c1@example.test> <c3@example.test>\r\nContent-Type: multipart/mixed; boundary=b\r\n\r\n";

    #[test]
    fn a_letter_is_kept_and_read_again() {
        let dir = std::env::temp_dir().join(format!("wakuwaku-store-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut s = Store::load(dir.clone(), &server("me"));
        s.reset(Folder::Inbox, "INBOX", 7);
        let e = Entry::from_head(4, (false, true, false), 900, Some(1_791_500_000), HEAD);
        assert_eq!((e.subject.as_str(), e.id.as_str(), e.parent.as_str(), e.refs.len(), e.attached), ("合同", "c4@vendor.example", "c3@example.test", 2, true));
        assert_eq!(e.from, Person { name: "Zhang San".into(), address: "zs@vendor.example".into() });
        s.add(Folder::Inbox, e);
        let body = [HEAD, b"--b\r\nContent-Type: text/plain\r\n\r\nline one\r\n\r\nline   two\r\n--b--\r\n"].concat();
        s.keep_body(Folder::Inbox, 4, &body);
        s.save();
        let again = Store::load(dir.clone(), &server("me"));
        let e = &again.inbox.letters[&4];
        assert_eq!((again.inbox.validity, e.flagged, e.body, e.snippet.as_str()), (7, true, true, "line one line two"));
        assert_eq!(again.body(Folder::Inbox, 4).unwrap(), body);
        assert_eq!(again.missing(), []);
        // Another user's: begun again, files gone.
        let other = Store::load(dir.clone(), &server("you"));
        assert!(other.inbox.letters.is_empty() && !dir.join("inbox").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_servers_flags_are_taken_but_not_over_ones_just_changed() {
        let dir = std::env::temp_dir().join(format!("wakuwaku-flags-{}", std::process::id()));
        let mut s = Store::load(dir.clone(), &server("me"));
        for uid in [1, 2, 3] {
            s.add(Folder::Inbox, Entry { uid, ..Default::default() });
        }
        s.set_flag(Folder::Inbox, 2, "flagged", true);
        let f = |uid, seen| Flags { uid, seen, flagged: false, answered: false };
        // 1 read elsewhere, 2 starred here (the server not told yet), 3 gone, 5 and 4 new.
        let new = s.take_flags(Folder::Inbox, &[f(1, true), f(2, false), f(4, false), f(5, false)]);
        assert_eq!(new, [5, 4]);
        assert!(s.inbox.letters[&1].seen && s.inbox.letters[&2].flagged && !s.inbox.letters.contains_key(&3));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
