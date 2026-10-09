// IMAP through io-imap, the library under himalaya (pimalaya): we open the
// connection (TCP, the system's TLS, STARTTLS) and it speaks the protocol.
// Only what the pet does: sign in, open the inbox, count the unread, list
// letters by their headers, read one whole without marking it read, mark
// one read, and IDLE.
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
use io_imap::rfc3501::fetch::ImapMessageFetchOptions;
use io_imap::rfc3501::login::{ImapLoginError, ImapLoginOptions};
use io_imap::rfc3501::search::ImapMessageSearchOptions;
use io_imap::rfc3501::store::ImapMessageStoreOptions;
use io_imap::sasl::auth_plain::{ImapAuthPlainError, ImapAuthPlainOptions};
use io_imap::types::core::{AString, IString, NString, Vec1};
use io_imap::types::fetch::{MessageDataItem, MessageDataItemName, Section};
use io_imap::types::flag::{Flag, FlagFetch, StoreType};
use io_imap::types::mailbox::Mailbox;
use io_imap::types::response::Capability;
use io_imap::types::search::SearchKey;
use io_imap::types::sequence::SequenceSet;
use serde_json::{json, Value};

use super::{Auth, Security, Server};

// How long a read or a write may wait once connected; in IDLE, how often
// the wait wakes (to refresh it when it is due).
const IO_WAIT: Duration = Duration::from_secs(20);
const IDLE_WAKE: Duration = Duration::from_secs(60);

#[derive(Debug, PartialEq)]
pub enum Fail {
    // Not reached (no such host, nothing on that port, no answer).
    Connect(String),
    // The encrypted connection did not come up.
    Tls(String),
    // Something answered, not as IMAP.
    NotImap(String),
    // No STARTTLS where it was asked for.
    NoStartTls,
    // The server said no to the user and password.
    Login(String),
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
            Fail::NoStartTls => ("noStartTls", ""),
            Fail::Login(t) => ("login", t.as_str()),
            Fail::Refused(t) => ("refused", t.as_str()),
            Fail::Other(t) => ("other", t.as_str()),
        };
        json!({ "kind": kind, "text": text })
    }
}

fn io_fail(e: io::Error) -> Fail {
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
enum Stream {
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

fn tls(host: &str, tcp: TcpStream) -> Result<Stream, Fail> {
    let connector = native_tls::TlsConnector::new().map_err(|e| Fail::Tls(e.to_string()))?;
    connector.connect(host, tcp).map(|s| Stream::Tls(Box::new(s))).map_err(|e| Fail::Tls(e.to_string()))
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
        let opened = if write { self.client.select(Mailbox::Inbox, Default::default()) } else { self.client.examine(Mailbox::Inbox, Default::default()) };
        Ok(opened.map_err(fail)?.exists.unwrap_or(0))
    }

    // The unread letters' UIDs.
    pub fn unseen(&mut self) -> Result<Vec<u32>, Fail> {
        let found = self.client.search(Vec1::from(SearchKey::Unseen), ImapMessageSearchOptions { uid: true }).map_err(fail)?;
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
                        MessageDataItem::Flags(flags) => head.seen = flags.iter().any(|f| matches!(f, FlagFetch::Flag(Flag::Seen))),
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

    // A letter whole, as it came, without marking it read; None when there
    // is no such UID.
    pub fn letter(&mut self, uid: u32) -> Result<Option<Vec<u8>>, Fail> {
        let set = SequenceSet::try_from(uid.to_string().as_str()).map_err(|e| Fail::Other(e.to_string()))?;
        let items = vec![MessageDataItemName::BodyExt { section: None, partial: None, peek: true }];
        let got = self.client.fetch(set, items.into(), ImapMessageFetchOptions { uid: true, ..Default::default() }).map_err(fail)?;
        Ok(got.into_values().flatten().find_map(|item| match item {
            MessageDataItem::BodyExt { section: None, data: NString(Some(s)), .. } => Some(s.as_ref().to_vec()),
            _ => None,
        }))
    }

    // A letter marked read (the inbox opened with write).
    pub fn mark_seen(&mut self, uid: u32) -> Result<(), Fail> {
        let set = SequenceSet::try_from(uid.to_string().as_str()).map_err(|e| Fail::Other(e.to_string()))?;
        self.client.store(set, StoreType::Add, vec![Flag::Seen], ImapMessageStoreOptions { uid: true }).map_err(fail)?;
        Ok(())
    }

    pub fn noop(&mut self) -> Result<(), Fail> {
        self.client.noop().map_err(fail)
    }

    // Waits for the server to say something changed (true), or `wait` to
    // pass (false); the IDLE ended either way. A socket shut down from
    // elsewhere ends it at once, as a failure.
    pub fn idle(&mut self, wait: Duration) -> Result<bool, Fail> {
        let done = Arc::new(AtomicBool::new(false));
        let mut idle = ImapIdle::new(done.clone(), ImapIdleOptions { timeout: Some(wait) });
        let _ = self.socket.set_read_timeout(Some(IDLE_WAKE.min(wait)));
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
                    // Quiet a while: a chance to refresh the IDLE when it is due.
                    Err(e) if matches!(e.kind(), io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock) => {}
                    Err(e) => break Err(io_fail(e)),
                },
                ImapCoroutineState::Complete(Ok(())) => break Ok(changed),
                ImapCoroutineState::Complete(Err(e)) => break Err(Fail::Other(e.to_string())),
            }
        };
        let _ = self.socket.set_read_timeout(Some(IO_WAIT));
        result
    }

    pub fn logout(mut self) {
        let _ = self.client.logout();
    }
}

// Connected and signed in.
pub fn session(server: &Server, password: &str, timeout: Duration) -> Result<Session, Fail> {
    let mut s = Session::open(server, timeout)?;
    s.login(&server.username, password, server.auth)?;
    Ok(s)
}
