// IMAP through io-imap, the library under himalaya (pimalaya): we open the
// connection (TCP, the system's TLS, STARTTLS) and it speaks the protocol.
// Only what the pet does: sign in, open the inbox, count the unread, list
// letters by their headers, read one whole without marking it read, mark
// one read (or answered), IDLE, and keep a letter sent in the Sent folder.
// The connection is the SMTP client's too (smtp.rs).
use std::any::Any;
use std::io::{self, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use io_imap::client::{ImapClient, ImapClientError, ImapClientStd, ImapStream};
use io_imap::coroutine::{ImapCoroutine, ImapCoroutineState};
use io_imap::rfc2177::idle::{ImapIdle, ImapIdleOptions, ImapIdleYield};
use io_imap::rfc2971::id::ImapServerIdOptions;
use io_imap::rfc3501::append::ImapMessageAppendOptions;
use io_imap::rfc3501::fetch::ImapMessageFetchOptions;
use io_imap::rfc3501::login::{ImapLoginError, ImapLoginOptions};
use io_imap::rfc3501::search::ImapMessageSearchOptions;
use io_imap::rfc3501::store::ImapMessageStoreOptions;
use io_imap::sasl::auth_plain::{ImapAuthPlainError, ImapAuthPlainOptions};
use io_imap::types::core::{AString, IString, NString, Vec1};
use io_imap::types::fetch::{MessageDataItem, MessageDataItemName, Section};
use io_imap::types::flag::{Flag, FlagFetch, StoreType};
use io_imap::types::mailbox::{ListMailbox, Mailbox};
use io_imap::types::response::Capability;
use io_imap::types::search::SearchKey;
use io_imap::types::sequence::SequenceSet;
use serde_json::{json, Value};

use super::{Auth, Security, Server};

// How long a read or a write may wait once connected; in IDLE, how often
// the wait wakes (to refresh it when it is due).
const IO_WAIT: Duration = Duration::from_secs(20);
// (and to see whether it is asked to stop waiting, idle's `kick`).
const IDLE_WAKE: Duration = Duration::from_secs(5);

#[derive(Debug, PartialEq)]
pub enum Fail {
    // Not reached (no such host, nothing on that port, no answer).
    Connect(String),
    // The encrypted connection did not come up.
    Tls(String),
    // Something answered, not as IMAP.
    NotImap(String),
    // Something answered, not as SMTP (smtp.rs).
    NotSmtp(String),
    // No STARTTLS where it was asked for.
    NoStartTls,
    // The server said no to the user and password.
    Login(String),
    // The outgoing server would not take a letter to someone: who, and its words.
    Recipient(String),
    // The server said no (NO, BAD) to something else.
    Refused(String),
    // The connection broke, or something else on the way.
    Other(String),
}

impl Fail {
    pub fn json(&self) -> Value {
        let (kind, text) = match self {
            Fail::Connect(t) => ("connect", t.as_str()),
            Fail::Tls(t) => ("tls", t.as_str()),
            Fail::NotImap(t) => ("notImap", t.as_str()),
            Fail::NotSmtp(t) => ("notSmtp", t.as_str()),
            Fail::NoStartTls => ("noStartTls", ""),
            Fail::Login(t) => ("login", t.as_str()),
            Fail::Recipient(t) => ("recipient", t.as_str()),
            Fail::Refused(t) => ("refused", t.as_str()),
            Fail::Other(t) => ("other", t.as_str()),
        };
        json!({ "kind": kind, "text": text })
    }
}

pub(super) fn io_fail(e: io::Error) -> Fail {
    Fail::Other(e.to_string())
}

// What io-imap said went wrong: the connection, or the server saying no.
fn fail(e: ImapClientError) -> Fail {
    match e {
        ImapClientError::Io(e) => io_fail(e),
        e => Fail::Refused(e.to_string()),
    }
}

// The connection: plain (until STARTTLS) or through the system's TLS; Gone
// for the moment it is being upgraded.
pub(super) enum Stream {
    Plain(TcpStream),
    Tls(Box<native_tls::TlsStream<TcpStream>>),
    Gone,
}

fn gone() -> io::Error {
    io::Error::new(io::ErrorKind::NotConnected, "the connection is gone")
}

impl Read for Stream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Stream::Plain(s) => s.read(buf),
            Stream::Tls(s) => s.read(buf),
            Stream::Gone => Err(gone()),
        }
    }
}

impl Write for Stream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Stream::Plain(s) => s.write(buf),
            Stream::Tls(s) => s.write(buf),
            Stream::Gone => Err(gone()),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Stream::Plain(s) => s.flush(),
            Stream::Tls(s) => s.flush(),
            Stream::Gone => Err(gone()),
        }
    }
}

impl ImapStream for Stream {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        match self {
            Stream::Plain(s) => s.set_read_timeout(timeout),
            Stream::Tls(s) => s.get_ref().set_read_timeout(timeout),
            Stream::Gone => Ok(()),
        }
    }
}

pub(super) fn tls(host: &str, tcp: TcpStream) -> Result<Stream, Fail> {
    let connector = native_tls::TlsConnector::new().map_err(|e| Fail::Tls(e.to_string()))?;
    connector.connect(host, tcp).map(|s| Stream::Tls(Box::new(s))).map_err(|e| Fail::Tls(e.to_string()))
}

// A server reached: the connection (encrypted at once for SSL/TLS), and its
// socket, to be shut down from elsewhere.
pub(super) fn connect(server: &Server, timeout: Duration) -> Result<(Stream, TcpStream), Fail> {
    let addr = (server.host.as_str(), server.port)
        .to_socket_addrs()
        .map_err(|e| Fail::Connect(e.to_string()))?
        .next()
        .ok_or_else(|| Fail::Connect("no address".into()))?;
    let tcp = TcpStream::connect_timeout(&addr, timeout).map_err(|e| Fail::Connect(e.to_string()))?;
    let _ = tcp.set_read_timeout(Some(timeout.max(IO_WAIT)));
    let _ = tcp.set_write_timeout(Some(timeout.max(IO_WAIT)));
    let socket = tcp.try_clone().map_err(io_fail)?;
    let stream = if server.security == Security::Ssl { tls(&server.host, tcp)? } else { Stream::Plain(tcp) };
    Ok((stream, socket))
}

fn caps_of(list: &[Capability]) -> Vec<String> {
    list.iter().map(|c| c.to_string().to_ascii_uppercase()).collect()
}

// Who is asking, for servers that want to know (163, 126 and yeah.net will
// not open a folder before).
fn our_id() -> Vec<(IString<'static>, NString<'static>)> {
    [("name", "Wakuwaku"), ("version", env!("CARGO_PKG_VERSION")), ("vendor", "Wakuwaku")]
        .into_iter()
        .filter_map(|(k, v)| Some((IString::try_from(k).ok()?, NString(Some(IString::try_from(v).ok()?)))))
        .collect()
}

// A letter as the list shows it: its headers (the fields asked for), and
// what the server knows of it.
#[derive(Debug, Default)]
pub struct Head {
    pub uid: u32,
    pub seen: bool,
    // Starred (\Flagged); replied to (\Answered).
    pub flagged: bool,
    pub answered: bool,
    pub size: u32,
    // When the server got it (seconds), for letters whose Date says nothing.
    pub received: Option<i64>,
    pub header: Vec<u8>,
}

pub struct Session {
    client: ImapClientStd,
    // The same socket: shut down from elsewhere to stop.
    pub socket: TcpStream,
    caps: Vec<String>,
}

impl Session {
    // Connected, encrypted as asked, the greeting read, and what it can do.
    pub fn open(server: &Server, timeout: Duration) -> Result<Session, Fail> {
        let (stream, socket) = connect(server, timeout)?;
        let mut client = ImapClientStd::new(stream);
        let greeting = client.greeting().map_err(|e| Fail::NotImap(e.to_string()))?;
        let mut s = Session { client, socket, caps: caps_of(&greeting.capability) };
        if server.security == Security::StartTls {
            if !s.has("STARTTLS") {
                return Err(Fail::NoStartTls);
            }
            // Bytes the server sent after its OK, before the handshake,
            // would be someone's in the middle: no.
            if !s.client.starttls().map_err(fail)?.is_empty() {
                return Err(Fail::Tls("data before the TLS handshake".into()));
            }
            let plain = s.client.stream.as_any_mut().downcast_mut::<Stream>().map(|st| std::mem::replace(st, Stream::Gone));
            let Some(Stream::Plain(tcp)) = plain else { return Err(Fail::Other("already encrypted".into())) };
            s.client.set_stream(tls(&server.host, tcp)?);
            s.caps = caps_of(&s.client.capability().map_err(fail)?);
        }
        Ok(s)
    }

    pub fn has(&self, cap: &str) -> bool {
        self.caps.iter().any(|c| c.eq_ignore_ascii_case(cap))
    }

    // What the server can do, as it said before signing in.
    pub fn caps(&self) -> &[String] {
        &self.caps
    }

    // Signed in as `auth` says: AUTHENTICATE PLAIN (any password goes), or
    // LOGIN, or (auto) PLAIN where it is offered, else LOGIN. The
    // credentials go after the server asks, never inline, as Coremail (163,
    // 126) says it takes them inline and does not. Then who we are, where
    // the server asks to be told.
    pub fn login(&mut self, user: &str, password: &str, auth: Auth) -> Result<(), Fail> {
        let plain = match auth {
            Auth::Plain => true,
            Auth::Login => false,
            Auth::Auto => self.has("AUTH=PLAIN"),
        };
        let caps = if plain {
            let opts = ImapAuthPlainOptions { initial_request: false, ensure_capabilities: true, auto_id: None };
            self.client.auth_plain(None, user, password, opts)
        } else if self.has("LOGINDISABLED") {
            return Err(Fail::Login("LOGINDISABLED".into()));
        } else {
            self.client.login(user, password, ImapLoginOptions { ensure_capabilities: true, auto_id: None })
        };
        let caps = caps.map_err(|e| match e {
            ImapClientError::AuthPlain(ImapAuthPlainError::No(why) | ImapAuthPlainError::Bad(why))
            | ImapClientError::Login(ImapLoginError::No(why) | ImapLoginError::Bad(why)) => Fail::Login(why),
            e => fail(e),
        })?;
        if !caps.is_empty() {
            self.caps = caps_of(&caps);
        }
        if self.has("ID") {
            let _ = self.client.id(ImapServerIdOptions { parameters: Some(our_id()) });
        }
        Ok(())
    }

    // The inbox opened, read-only or not; how many letters it holds.
    pub fn inbox(&mut self, write: bool) -> Result<u32, Fail> {
        Ok(self.folder(Mailbox::Inbox, write)?.exists)
    }

    // A folder opened, read-only (EXAMINE) or not (SELECT): how many letters
    // it holds, and its UIDVALIDITY (another one, and its UIDs are not the
    // ones kept: store.rs starts it again).
    pub fn folder(&mut self, mailbox: Mailbox<'static>, write: bool) -> Result<Opened, Fail> {
        let opened = if write { self.client.select(mailbox, Default::default()) } else { self.client.examine(mailbox, Default::default()) };
        let data = opened.map_err(fail)?;
        Ok(Opened { exists: data.exists.unwrap_or(0), validity: data.uid_validity.map_or(0, |v| v.get()) })
    }

    // Every letter of the folder open: its UID and flags (UID FETCH 1:*).
    pub fn flags_all(&mut self) -> Result<Vec<Flags>, Fail> {
        let set = SequenceSet::try_from("1:*").map_err(|e| Fail::Other(e.to_string()))?;
        let items = vec![MessageDataItemName::Uid, MessageDataItemName::Flags];
        let got = self.client.fetch(set, items.into(), ImapMessageFetchOptions { uid: true, ..Default::default() }).map_err(fail)?;
        Ok(got
            .into_values()
            .map(|items| {
                let mut f = Flags::default();
                for item in items {
                    match item {
                        MessageDataItem::Uid(n) => f.uid = n.get(),
                        MessageDataItem::Flags(flags) => {
                            let has = |want: Flag| flags.iter().any(|x| matches!(x, FlagFetch::Flag(f) if *f == want));
                            (f.seen, f.flagged, f.answered) = (has(Flag::Seen), has(Flag::Flagged), has(Flag::Answered));
                        }
                        _ => {}
                    }
                }
                f
            })
            .filter(|f| f.uid > 0)
            .collect())
    }

    // The UIDs above one (letters come in since).
    pub fn newer(&mut self, after: u32) -> Result<Vec<u32>, Fail> {
        let set = SequenceSet::try_from(format!("{}:*", after.saturating_add(1)).as_str()).map_err(|e| Fail::Other(e.to_string()))?;
        // "n:*" finds the last letter when there is none above n.
        Ok(self.search(SearchKey::Uid(set))?.into_iter().filter(|&uid| uid > after).collect())
    }

    // The unread letters' UIDs.
    pub fn unseen(&mut self) -> Result<Vec<u32>, Fail> {
        self.search(SearchKey::Unseen)
    }

    // The UIDs of the letters a search key finds (unread, starred…).
    pub fn search(&mut self, key: SearchKey<'static>) -> Result<Vec<u32>, Fail> {
        let found = self.client.search(Vec1::from(key), ImapMessageSearchOptions { uid: true }).map_err(fail)?;
        Ok(found.into_iter().map(|n| n.get()).collect())
    }

    // The letters in a set ("41:50" by position, or UIDs), by the header
    // fields named and what the server knows, none marked read.
    pub fn heads(&mut self, set: &str, uid: bool, fields: &[&str]) -> Result<Vec<Head>, Fail> {
        let set = SequenceSet::try_from(set).map_err(|e| Fail::Other(e.to_string()))?;
        let names: Vec<AString<'static>> = fields.iter().filter_map(|f| AString::try_from(f.to_string()).ok()).collect();
        let names: Vec1<AString<'static>> = Vec1::try_from(names).map_err(|_| Fail::Other("no header fields".into()))?;
        let items = vec![
            MessageDataItemName::Uid,
            MessageDataItemName::Flags,
            MessageDataItemName::InternalDate,
            MessageDataItemName::Rfc822Size,
            MessageDataItemName::BodyExt { section: Some(Section::HeaderFields(None, names)), partial: None, peek: true },
        ];
        let got = self.client.fetch(set, items.into(), ImapMessageFetchOptions { uid, ..Default::default() }).map_err(fail)?;
        Ok(got
            .into_values()
            .map(|items| {
                let mut head = Head::default();
                for item in items {
                    match item {
                        MessageDataItem::Uid(n) => head.uid = n.get(),
                        MessageDataItem::Flags(flags) => {
                            head.seen = flags.iter().any(|f| matches!(f, FlagFetch::Flag(Flag::Seen)));
                            head.flagged = flags.iter().any(|f| matches!(f, FlagFetch::Flag(Flag::Flagged)));
                            head.answered = flags.iter().any(|f| matches!(f, FlagFetch::Flag(Flag::Answered)));
                        }
                        MessageDataItem::Rfc822Size(n) => head.size = n,
                        MessageDataItem::InternalDate(d) => head.received = Some(d.as_ref().timestamp()),
                        MessageDataItem::BodyExt { data: NString(Some(s)), .. } => head.header = s.as_ref().to_vec(),
                        _ => {}
                    }
                }
                head
            })
            .filter(|h| h.uid > 0)
            .collect())
    }

    // A letter whole, as it came, and whether it is starred, without marking
    // it read; None when there is no such UID.
    pub fn letter(&mut self, uid: u32) -> Result<Option<(Vec<u8>, bool)>, Fail> {
        let set = SequenceSet::try_from(uid.to_string().as_str()).map_err(|e| Fail::Other(e.to_string()))?;
        let items = vec![MessageDataItemName::Flags, MessageDataItemName::BodyExt { section: None, partial: None, peek: true }];
        let got = self.client.fetch(set, items.into(), ImapMessageFetchOptions { uid: true, ..Default::default() }).map_err(fail)?;
        let (mut raw, mut flagged) = (None, false);
        for item in got.into_values().flatten() {
            match item {
                MessageDataItem::BodyExt { section: None, data: NString(Some(s)), .. } => raw = Some(s.as_ref().to_vec()),
                MessageDataItem::Flags(flags) => flagged = flags.iter().any(|f| matches!(f, FlagFetch::Flag(Flag::Flagged))),
                _ => {}
            }
        }
        Ok(raw.map(|r| (r, flagged)))
    }

    // A flag (read, starred) put on a letter or taken off (the inbox opened with write).
    pub fn set_flag(&mut self, uid: u32, flag: Flag<'static>, on: bool) -> Result<(), Fail> {
        let set = SequenceSet::try_from(uid.to_string().as_str()).map_err(|e| Fail::Other(e.to_string()))?;
        let how = if on { StoreType::Add } else { StoreType::Remove };
        self.client.store(set, how, vec![flag], ImapMessageStoreOptions { uid: true }).map_err(fail)?;
        Ok(())
    }

    // The folder sent letters are kept in: the one the server marks \Sent
    // (SPECIAL-USE), else one named as mail programs name it; None when
    // there is neither.
    pub fn sent_box(&mut self) -> Result<Option<Mailbox<'static>>, Fail> {
        let reference = Mailbox::try_from("").map_err(|e| Fail::Other(e.to_string()))?;
        let pattern = ListMailbox::try_from("*").map_err(|e| Fail::Other(e.to_string()))?;
        let all = self.client.list(reference, pattern).map_err(fail)?;
        let has = |attrs: &[io_imap::types::flag::FlagNameAttribute], name: &str| attrs.iter().any(|a| a.to_string().eq_ignore_ascii_case(name));
        if let Some((m, ..)) = all.iter().find(|(_, _, attrs)| has(attrs, "\\Sent")) {
            return Ok(Some(m.clone()));
        }
        let open: Vec<&Mailbox<'static>> = all.iter().filter(|(_, _, attrs)| !has(attrs, "\\Noselect")).map(|(m, ..)| m).collect();
        Ok(SENT_NAMES.iter().find_map(|name| open.iter().find(|m| leaf(m).eq_ignore_ascii_case(name))).map(|m| (*m).clone()))
    }

    // A letter put into a folder, marked read; its UID there, where the
    // server says (UIDPLUS's APPENDUID).
    pub fn append(&mut self, mailbox: Mailbox<'static>, raw: &[u8]) -> Result<Option<u32>, Fail> {
        let opts = ImapMessageAppendOptions { flags: vec![Flag::Seen], ..Default::default() };
        let (_, uid) = self.client.append(mailbox, raw, opts).map_err(fail)?;
        Ok(uid.map(|(_, uid)| uid))
    }

    pub fn noop(&mut self) -> Result<(), Fail> {
        self.client.noop().map_err(fail)
    }

    // Waits for the server to say something changed (true), or `wait` to
    // pass, or `kick` to be set (false); the IDLE ended either way. A socket
    // shut down from elsewhere ends it at once, as a failure.
    pub fn idle(&mut self, wait: Duration, kick: &AtomicBool) -> Result<bool, Fail> {
        let done = Arc::new(AtomicBool::new(false));
        let mut idle = ImapIdle::new(done.clone(), ImapIdleOptions { timeout: Some(wait) });
        // On the stream read from: one set through the copy of the socket
        // does not hold on Windows (it waited IO_WAIT).
        let _ = self.client.stream.set_read_timeout(Some(IDLE_WAKE.min(wait)));
        let mut buf = vec![0u8; 16 * 1024];
        let mut got: Option<usize> = None;
        let mut changed = false;
        let result = loop {
            match idle.resume(&mut self.client.fragmentizer, got.take().map(|n| &buf[..n])) {
                ImapCoroutineState::Yielded(ImapIdleYield::Event(_)) => {
                    changed = true;
                    done.store(true, Ordering::SeqCst);
                }
                ImapCoroutineState::Yielded(ImapIdleYield::WantsWrite(bytes)) => {
                    if let Err(e) = self.client.stream.write_all(&bytes) {
                        break Err(io_fail(e));
                    }
                }
                ImapCoroutineState::Yielded(ImapIdleYield::WantsRead) => match self.client.stream.read(&mut buf) {
                    Ok(0) => break Err(Fail::Other("the server closed the connection".into())),
                    Ok(n) => got = Some(n),
                    // Quiet a while: a chance to refresh the IDLE when it is
                    // due, or to stop when asked to.
                    Err(e) if matches!(e.kind(), io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock) => {
                        if kick.load(Ordering::SeqCst) {
                            done.store(true, Ordering::SeqCst);
                        }
                    }
                    Err(e) => break Err(io_fail(e)),
                },
                ImapCoroutineState::Complete(Ok(())) => break Ok(changed),
                ImapCoroutineState::Complete(Err(e)) => break Err(Fail::Other(e.to_string())),
            }
        };
        let _ = self.client.stream.set_read_timeout(Some(IO_WAIT));
        result
    }

    pub fn logout(mut self) {
        let _ = self.client.logout();
    }
}

// The names a Sent folder goes by where the server does not mark it: the
// English ones (Gmail's "[Gmail]/Sent Mail", Exchange's "Sent Items", QQ's
// "Sent Messages"), German (Exchange's), Chinese (163, 126).
const SENT_NAMES: [&str; 9] = ["Sent", "Sent Items", "Sent Messages", "Sent Mail", "Gesendete Elemente", "Gesendete Objekte", "Gesendet", "已发送", "已发送邮件"];

// A folder opened: how many letters, its UIDVALIDITY.
pub struct Opened {
    pub exists: u32,
    pub validity: u32,
}

// A letter's UID and flags.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Flags {
    pub uid: u32,
    pub seen: bool,
    pub flagged: bool,
    pub answered: bool,
}

// A folder by its whole name (as LIST gave it, kept in the store), and back.
pub fn mailbox(name: &str) -> Option<Mailbox<'static>> {
    if name.eq_ignore_ascii_case("INBOX") {
        return Some(Mailbox::Inbox);
    }
    Mailbox::try_from(name.to_string()).ok()
}

pub fn name_of(m: &Mailbox) -> String {
    match m {
        Mailbox::Inbox => "INBOX".into(),
        Mailbox::Other(o) => String::from_utf8_lossy(o.as_ref()).into_owned(),
    }
}

// A folder's own name, the part after its parents (INBOX.Sent is Sent).
fn leaf(m: &Mailbox) -> String {
    let name = match m {
        Mailbox::Inbox => return "INBOX".into(),
        Mailbox::Other(o) => String::from_utf8_lossy(o.as_ref()).into_owned(),
    };
    name.rsplit(['/', '.']).next().unwrap_or("").to_string()
}

// Connected and signed in.
pub fn session(server: &Server, password: &str, timeout: Duration) -> Result<Session, Fail> {
    let mut s = Session::open(server, timeout)?;
    s.login(&server.username, password, server.auth)?;
    Ok(s)
}
