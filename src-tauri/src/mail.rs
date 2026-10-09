// Mail in the island, built in. Accounts are set up in the settings the way
// Thunderbird does it: an address and a password, then the server looked up
// (the domain's own autoconfig, Thunderbird's database ISPDB, the database's
// entry for the provider of the domain's MX host, then a guess) or typed in.
// Each account that is on is watched over IMAP by a thread of its own: the
// unread count and the newest unread letter (who, what about) as a widget,
// and the island opened for a new one, at once where the server pushes
// (IDLE), else every minute. Read only: the inbox is EXAMINEd and headers
// are read with BODY.PEEK, so nothing is marked read. Passwords are kept in
// Windows' Credential Manager ("Wakuwaku mail <id>"), never in the settings.
//
// Settings: "mail": [{ id, address, host, port, security: ssl | starttls |
// plain, username, on }], changed only through the commands below.
use std::collections::HashMap;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{Shutdown, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use base64::Engine;
use serde_json::{json, Value};
use tauri::AppHandle;

use crate::{i18n, now_ms, settings, shared, Shared};

// How long a connection may take, how long the server may be quiet while it
// pushes (then the push is asked for again), how often one that cannot push
// is asked.
const CONNECT: Duration = Duration::from_secs(15);
const IDLE_FOR: Duration = Duration::from_secs(9 * 60);
const POLL: Duration = Duration::from_secs(60);

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Security {
    Ssl,
    StartTls,
    Plain,
}

impl Security {
    fn parse(s: &str) -> Option<Security> {
        match s {
            "ssl" => Some(Security::Ssl),
            "starttls" => Some(Security::StartTls),
            "plain" => Some(Security::Plain),
            _ => None,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Security::Ssl => "ssl",
            Security::StartTls => "starttls",
            Security::Plain => "plain",
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Server {
    pub host: String,
    pub port: u16,
    pub security: Security,
    pub username: String,
}

impl Server {
    fn json(&self) -> Value {
        json!({ "host": self.host, "port": self.port, "security": self.security.name(), "username": self.username })
    }

    // As the page sent it, checked: a host name, a port, a way, a user.
    fn from_json(v: &Value) -> Option<Server> {
        let host = v["host"].as_str()?.trim().to_ascii_lowercase();
        let is_host = !host.is_empty() && host.len() <= 253 && host.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-');
        let port = v["port"].as_u64().or_else(|| v["port"].as_str()?.trim().parse().ok()).filter(|p| (1..=65535).contains(p))? as u16;
        let username = v["username"].as_str()?.trim();
        let is_user = !username.is_empty() && username.chars().count() <= 320 && !username.chars().any(char::is_control);
        (is_host && is_user).then(|| Server { host, port, security: Security::parse(v["security"].as_str().unwrap_or("")).unwrap_or(Security::Ssl), username: username.into() })
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Account {
    pub id: String,
    pub address: String,
    pub server: Server,
    pub on: bool,
}

impl Account {
    fn json(&self) -> Value {
        let mut v = self.server.json();
        v["id"] = json!(self.id);
        v["address"] = json!(self.address);
        v["on"] = json!(self.on);
        v
    }
}

fn accounts(sh: &Shared) -> Vec<Account> {
    let all = sh.setting("mail");
    all.as_array()
        .into_iter()
        .flatten()
        .filter_map(|a| {
            Some(Account {
                id: a["id"].as_str()?.into(),
                address: a["address"].as_str()?.into(),
                server: Server::from_json(a)?,
                on: a["on"] != false,
            })
        })
        .collect()
}

// local@domain, the domain in lower case.
fn split(address: &str) -> Option<(&str, String)> {
    let (local, domain) = address.trim().rsplit_once('@')?;
    let ok = !local.is_empty() && local.len() <= 64 && !local.chars().any(|c| c.is_whitespace() || c.is_control());
    let domain = domain.to_ascii_lowercase();
    let ok_domain = domain.contains('.') && domain.len() <= 253 && domain.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-');
    (ok && ok_domain).then_some((local, domain))
}

// --- Finding the server ---------------------------------------------------------------------

// What was found for an address: the server, and where it came from
// (autoconfig, wellknown, ispdb, mx, guess); oauth when the provider takes
// only a browser sign-in for IMAP.
pub fn discover(address: &str) -> Value {
    let Some((_, domain)) = split(address) else { return json!({ "ok": false, "error": "address" }) };
    let address = address.trim();
    let agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(6))).build().new_agent();
    let get = |url: &str| -> Option<String> {
        let mut res = agent.get(url).header("user-agent", "wakuwaku").call().ok()?;
        res.body_mut().with_config().limit(256 * 1024).read_to_string().ok()
    };
    let mut oauth = false;
    let configs = [
        ("autoconfig", format!("https://autoconfig.{domain}/mail/config-v1.1.xml?emailaddress={}", address.replace('@', "%40").replace('+', "%2B"))),
        ("wellknown", format!("https://{domain}/.well-known/autoconfig/mail/config-v1.1.xml")),
        ("ispdb", format!("https://autoconfig.thunderbird.net/v1.1/{domain}")),
    ];
    for (source, url) in configs {
        if let Some(xml) = get(&url) {
            match from_config(&xml, address) {
                Some(Ok(server)) => return json!({ "ok": true, "found": true, "source": source, "server": server.json() }),
                Some(Err(())) => oauth = true,
                None => {}
            }
        }
    }
    // The provider of the domain's mail host, as Thunderbird asks: a domain
    // of its own on Google's or Microsoft's mail, or a university's on a
    // research network's.
    for mx in mx_hosts(&domain).iter().take(2) {
        let base = base_domain(mx);
        if base == domain {
            continue;
        }
        if let Some(xml) = get(&format!("https://autoconfig.thunderbird.net/v1.1/{base}")) {
            match from_config(&xml, address) {
                Some(Ok(server)) => return json!({ "ok": true, "found": true, "source": "mx", "server": server.json() }),
                Some(Err(())) => oauth = true,
                None => {}
            }
        }
    }
    if oauth {
        return json!({ "ok": true, "found": false, "oauth": true });
    }
    // A guess, where something answers as IMAP.
    for host in [format!("imap.{domain}"), format!("mail.{domain}"), domain.clone()] {
        if let Some((security, port)) = probe(&host) {
            let server = Server { host, port, security, username: address.into() };
            return json!({ "ok": true, "found": true, "source": "guess", "server": server.json() });
        }
    }
    json!({ "ok": true, "found": false })
}

// The IMAP server in an autoconfig file: Some(Ok) one that takes a password,
// Some(Err) when they all take only OAuth, None when there is none.
fn from_config(xml: &str, address: &str) -> Option<Result<Server, ()>> {
    let (local, domain) = split(address)?;
    let fill = |s: &str| s.replace("%EMAILADDRESS%", address).replace("%EMAILLOCALPART%", local).replace("%EMAILDOMAIN%", &domain);
    let mut oauth_only = false;
    let mut rest = xml;
    while let Some(at) = rest.find("<incomingServer") {
        let block = &rest[at..];
        let end = block.find("</incomingServer>").unwrap_or(block.len());
        let (head, body) = block[..end].split_once('>').unwrap_or(("", ""));
        rest = &block[end..];
        if !head.contains("type=\"imap\"") {
            continue;
        }
        let auths: Vec<&str> = tags(body, "authentication");
        let by_password = auths.is_empty() || auths.iter().any(|a| matches!(*a, "password-cleartext" | "plain" | "password-encrypted" | "secure"));
        if !by_password {
            oauth_only |= auths.iter().any(|a| a.eq_ignore_ascii_case("OAuth2"));
            continue;
        }
        let security = match tags(body, "socketType").first().copied() {
            Some("SSL") => Security::Ssl,
            Some("STARTTLS") => Security::StartTls,
            _ => continue,
        };
        let host = fill(tags(body, "hostname").first()?);
        let port = tags(body, "port").first().and_then(|p| p.parse().ok()).unwrap_or(if security == Security::Ssl { 993 } else { 143 });
        let username = tags(body, "username").first().map(|u| fill(u)).unwrap_or_else(|| address.into());
        return Some(Ok(Server { host: host.to_ascii_lowercase(), port, security, username }));
    }
    oauth_only.then_some(Err(()))
}

// The text of each <name>…</name> in a piece of XML, entities read.
fn tags<'a>(xml: &'a str, name: &str) -> Vec<&'a str> {
    let (open, close) = (format!("<{name}>"), format!("</{name}>"));
    let mut found = Vec::new();
    let mut rest = xml;
    while let Some(at) = rest.find(&open) {
        let after = &rest[at + open.len()..];
        let Some(end) = after.find(&close) else { break };
        found.push(after[..end].trim());
        rest = &after[end..];
    }
    found
}

// The domain a mail host belongs to: mx1.qq.com is qq.com; a host under a
// country's com, edu, ac… keeps one label more.
fn base_domain(host: &str) -> String {
    let labels: Vec<&str> = host.trim_end_matches('.').split('.').collect();
    let n = labels.len();
    let keep = if n >= 3 && labels[n - 1].len() == 2 && matches!(labels[n - 2], "com" | "net" | "org" | "edu" | "gov" | "ac" | "co") { 3 } else { 2 };
    labels[n.saturating_sub(keep)..].join(".").to_ascii_lowercase()
}

// Something answering as IMAP at a host: over TLS on 993, else with
// STARTTLS on 143.
fn probe(host: &str) -> Option<(Security, u16)> {
    let try_one = |security, port| {
        let server = Server { host: host.into(), port, security, username: String::new() };
        Conn::open(&server, Duration::from_secs(4)).is_ok()
    };
    if try_one(Security::Ssl, 993) {
        return Some((Security::Ssl, 993));
    }
    try_one(Security::StartTls, 143).then_some((Security::StartTls, 143))
}

// --- IMAP: just what watching an inbox takes -------------------------------------------------

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
    fn json(&self) -> Value {
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

enum Stream {
    Plain(TcpStream),
    Tls(Box<native_tls::TlsStream<TcpStream>>),
}

impl Read for Stream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Stream::Plain(s) => s.read(buf),
            Stream::Tls(s) => s.read(buf),
        }
    }
}

impl Write for Stream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Stream::Plain(s) => s.write(buf),
            Stream::Tls(s) => s.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Stream::Plain(s) => s.flush(),
            Stream::Tls(s) => s.flush(),
        }
    }
}

fn tls(host: &str, tcp: TcpStream) -> Result<Stream, Fail> {
    let connector = native_tls::TlsConnector::new().map_err(|e| Fail::Tls(e.to_string()))?;
    connector.connect(host, tcp).map(|s| Stream::Tls(Box::new(s))).map_err(|e| Fail::Tls(e.to_string()))
}

// A response: its text (literals left out), and the literals in it.
type Response = (String, Vec<Vec<u8>>);

pub struct Conn {
    io: BufReader<Stream>,
    // The same socket: its timeouts, and shut down from elsewhere to stop.
    socket: TcpStream,
    // A line read in part when a read timed out.
    partial: Vec<u8>,
    next: u32,
    caps: Vec<String>,
}

impl Conn {
    // Connected, encrypted as asked, the greeting read, and what it can do.
    fn open(server: &Server, timeout: Duration) -> Result<Conn, Fail> {
        let addr = (server.host.as_str(), server.port)
            .to_socket_addrs()
            .map_err(|e| Fail::Connect(e.to_string()))?
            .next()
            .ok_or_else(|| Fail::Connect("no address".into()))?;
        let tcp = TcpStream::connect_timeout(&addr, timeout).map_err(|e| Fail::Connect(e.to_string()))?;
        let _ = tcp.set_read_timeout(Some(timeout.max(Duration::from_secs(20))));
        let _ = tcp.set_write_timeout(Some(timeout.max(Duration::from_secs(20))));
        let socket = tcp.try_clone().map_err(io_fail)?;
        let stream = if server.security == Security::Ssl { tls(&server.host, tcp)? } else { Stream::Plain(tcp) };
        let mut conn = Conn { io: BufReader::new(stream), socket, partial: Vec::new(), next: 1, caps: Vec::new() };
        let greeting = conn.line().map_err(|e| Fail::NotImap(e.to_string()))?;
        let greeting = String::from_utf8_lossy(&greeting).into_owned();
        if !(greeting.starts_with("* OK") || greeting.starts_with("* PREAUTH")) {
            return Err(Fail::NotImap(greeting.chars().take(80).collect()));
        }
        conn.capability()?;
        if server.security == Security::StartTls {
            if !conn.has("STARTTLS") {
                return Err(Fail::NoStartTls);
            }
            conn.command("STARTTLS")?;
            let Stream::Plain(tcp) = std::mem::replace(conn.io.get_mut(), Stream::Plain(conn.socket.try_clone().map_err(io_fail)?)) else {
                return Err(Fail::Other("already encrypted".into()));
            };
            conn.io = BufReader::new(tls(&server.host, tcp)?);
            conn.capability()?;
        }
        Ok(conn)
    }

    fn has(&self, cap: &str) -> bool {
        self.caps.iter().any(|c| c.eq_ignore_ascii_case(cap))
    }

    fn capability(&mut self) -> Result<(), Fail> {
        let lines = self.command("CAPABILITY")?;
        self.caps = lines.iter().filter_map(|(t, _)| t.strip_prefix("* CAPABILITY ")).flat_map(|t| t.split_whitespace().map(String::from)).collect();
        Ok(())
    }

    // A line, without its CRLF; a timeout keeps what came of it for the next call.
    fn line(&mut self) -> io::Result<Vec<u8>> {
        loop {
            match self.io.read_until(b'\n', &mut self.partial) {
                Ok(0) => return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "the server closed the connection")),
                Ok(_) if self.partial.ends_with(b"\n") => {
                    let mut line = std::mem::take(&mut self.partial);
                    while line.last().is_some_and(|b| *b == b'\n' || *b == b'\r') {
                        line.pop();
                    }
                    return Ok(line);
                }
                Ok(_) => continue,
                Err(e) => return Err(e),
            }
        }
    }

    fn response(&mut self) -> io::Result<Response> {
        let mut text = Vec::new();
        let mut literals = Vec::new();
        loop {
            let line = self.line()?;
            text.extend_from_slice(&line);
            match literal_len(&line) {
                Some(n) if n <= 4 << 20 => {
                    let mut buf = vec![0; n];
                    self.io.read_exact(&mut buf)?;
                    literals.push(buf);
                }
                _ => break,
            }
        }
        Ok((String::from_utf8_lossy(&text).into_owned(), literals))
    }

    fn send(&mut self, bytes: &[u8]) -> Result<(), Fail> {
        let s = self.io.get_mut();
        s.write_all(bytes).and_then(|_| s.flush()).map_err(io_fail)
    }

    fn tag(&mut self) -> String {
        self.next += 1;
        format!("w{}", self.next)
    }

    // The responses until the tagged one; an error for NO or BAD.
    fn finish(&mut self, tag: &str) -> Result<Vec<Response>, Fail> {
        let mut untagged = Vec::new();
        loop {
            let (text, literals) = self.response().map_err(io_fail)?;
            if let Some(rest) = text.strip_prefix(tag).and_then(|r| r.strip_prefix(' ')) {
                if rest.starts_with("OK") {
                    return Ok(untagged);
                }
                let why = rest.split_once(' ').map_or("", |(_, w)| w).trim().to_string();
                return Err(Fail::Refused(why));
            }
            untagged.push((text, literals));
        }
    }

    fn command(&mut self, cmd: &str) -> Result<Vec<Response>, Fail> {
        let tag = self.tag();
        self.send(format!("{tag} {cmd}\r\n").as_bytes())?;
        self.finish(&tag)
    }

    // Signed in: AUTHENTICATE PLAIN where it is offered (any password goes,
    // base64), else LOGIN. What it can do is asked again after.
    fn login(&mut self, user: &str, password: &str) -> Result<(), Fail> {
        let plain = base64::engine::general_purpose::STANDARD.encode(format!("\0{user}\0{password}"));
        let done = if self.has("AUTH=PLAIN") {
            let tag = self.tag();
            if self.has("SASL-IR") {
                self.send(format!("{tag} AUTHENTICATE PLAIN {plain}\r\n").as_bytes())?;
            } else {
                self.send(format!("{tag} AUTHENTICATE PLAIN\r\n").as_bytes())?;
                let (go, _) = self.response().map_err(io_fail)?;
                if !go.starts_with('+') {
                    return Err(Fail::Login(go));
                }
                self.send(format!("{plain}\r\n").as_bytes())?;
            }
            self.finish(&tag)
        } else if self.has("LOGINDISABLED") {
            return Err(Fail::Login("LOGINDISABLED".into()));
        } else {
            self.login_plainly(user, password)
        };
        done.map_err(|e| match e {
            Fail::Refused(why) => Fail::Login(why),
            e => e,
        })?;
        self.capability()
    }

    // LOGIN: each in quotes, or, beyond plain ASCII (a quoted string may not
    // hold it), as a literal the server first says go for.
    fn login_plainly(&mut self, user: &str, password: &str) -> Result<Vec<Response>, Fail> {
        let tag = self.tag();
        let mut line = format!("{tag} LOGIN");
        for value in [user, password] {
            if value.is_ascii() && !value.chars().any(|c| c.is_ascii_control()) {
                line += &format!(" {}", quoted(value));
                continue;
            }
            line += &format!(" {{{}}}\r\n", value.len());
            self.send(line.as_bytes())?;
            let (go, _) = self.response().map_err(io_fail)?;
            if !go.starts_with('+') {
                return Err(Fail::Refused(go));
            }
            self.send(value.as_bytes())?;
            line.clear();
        }
        self.send(format!("{line}\r\n").as_bytes())?;
        self.finish(&tag)
    }

    // Who is asking, for servers that want to know (163, 126 and yeah.net
    // will not open a folder before).
    fn id(&mut self) {
        if self.has("ID") {
            let _ = self.command(&format!("ID (\"name\" \"Wakuwaku\" \"version\" \"{}\" \"vendor\" \"Wakuwaku\")", env!("CARGO_PKG_VERSION")));
        }
    }

    // The unread letters' UIDs in the inbox, opened read-only.
    fn unseen(&mut self) -> Result<Vec<u32>, Fail> {
        let lines = self.command("UID SEARCH UNSEEN")?;
        Ok(lines.iter().filter_map(|(t, _)| t.strip_prefix("* SEARCH")).flat_map(|t| t.split_whitespace().filter_map(|n| n.parse().ok())).collect())
    }

    // Who a letter is from and what about, without marking it read.
    fn header(&mut self, uid: u32) -> Result<(String, String), Fail> {
        let lines = self.command(&format!("UID FETCH {uid} (BODY.PEEK[HEADER.FIELDS (FROM SUBJECT)])"))?;
        let head = lines.into_iter().flat_map(|(_, l)| l).next().unwrap_or_default();
        Ok(from_and_subject(&head))
    }

    // Waits for the server to say something changed (true), or IDLE_FOR to
    // pass (false); then ends the IDLE.
    fn idle(&mut self) -> Result<bool, Fail> {
        let tag = self.tag();
        self.send(format!("{tag} IDLE\r\n").as_bytes())?;
        let (go, _) = self.response().map_err(io_fail)?;
        if !go.starts_with('+') {
            return Err(Fail::Refused(go));
        }
        let _ = self.socket.set_read_timeout(Some(IDLE_FOR));
        let changed = loop {
            match self.response() {
                Ok((text, _)) if text.starts_with('*') && ["EXISTS", "EXPUNGE", "FETCH", "RECENT"].iter().any(|w| text.contains(w)) => break true,
                Ok(_) => continue,
                Err(e) if matches!(e.kind(), io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock) => break false,
                Err(e) => return Err(io_fail(e)),
            }
        };
        let _ = self.socket.set_read_timeout(Some(CONNECT.max(Duration::from_secs(20))));
        self.send(b"DONE\r\n")?;
        self.finish(&tag)?;
        Ok(changed)
    }
}

// "{123}" at the end of a line: a literal of so many bytes follows it.
fn literal_len(line: &[u8]) -> Option<usize> {
    let line = std::str::from_utf8(line).ok()?;
    let inner = line.strip_suffix('}')?.rsplit_once('{')?.1;
    inner.trim_end_matches('+').parse().ok()
}

// A quoted IMAP string.
fn quoted(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

// A header block's sender (the name, else the address) and subject, its
// encoded words and charsets read.
fn from_and_subject(head: &[u8]) -> (String, String) {
    let Some(message) = mail_parser::MessageParser::default().parse(head) else { return (String::new(), String::new()) };
    let from = message.from().and_then(|a| a.first()).and_then(|a| a.name().or(a.address())).unwrap_or("").trim().to_string();
    (from, message.subject().unwrap_or("").trim().to_string())
}

// Connected and signed in, the inbox open read-only.
fn session(server: &Server, password: &str) -> Result<Conn, Fail> {
    let mut conn = Conn::open(server, CONNECT)?;
    conn.login(&server.username, password)?;
    conn.id();
    conn.command("EXAMINE INBOX")?;
    Ok(conn)
}

// A try before an account is kept: signed in, and the unread counted.
fn test(server: &Server, password: &str) -> Result<usize, Fail> {
    let mut conn = session(server, password)?;
    let unread = conn.unseen()?.len();
    let _ = conn.command("LOGOUT");
    Ok(unread)
}

// --- Watching an inbox ------------------------------------------------------------------------

#[derive(Default)]
struct Watch {
    stop: AtomicBool,
    // The connection's socket, shut down to stop at once.
    socket: Mutex<Option<TcpStream>>,
    // connecting | ok | error, the unread count, what went wrong.
    status: Mutex<Value>,
}

impl Watch {
    fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(s) = self.socket.lock().unwrap().take() {
            let _ = s.shutdown(Shutdown::Both);
        }
    }

    fn stopped(&self) -> bool {
        self.stop.load(Ordering::SeqCst)
    }

    // A rest that ends early when stopped.
    fn rest(&self, d: Duration) {
        let until = Instant::now() + d;
        while !self.stopped() && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(250));
        }
    }

    fn set(&self, status: Value) {
        *self.status.lock().unwrap() = status;
    }
}

#[derive(Default)]
pub struct Runner {
    watches: HashMap<String, (Account, Arc<Watch>)>,
}

fn widget_id(account: &str) -> String {
    format!("inbox-{account}")
}

// Each account watched as it is set: started, stopped, started again when
// its settings changed. True when what the island shows changed.
pub fn sync(sh: &Arc<Shared>) -> bool {
    let want: Vec<Account> = accounts(sh).into_iter().filter(|a| a.on).collect();
    let mut gone = Vec::new();
    {
        let mut r = sh.mail.lock().unwrap();
        r.watches.retain(|id, (account, watch)| {
            let keep = want.iter().any(|a| a == account);
            if !keep {
                watch.stop();
                gone.push(id.clone());
            }
            keep
        });
        for account in want {
            if r.watches.contains_key(&account.id) {
                continue;
            }
            let watch = Arc::new(Watch::default());
            watch.set(json!({ "state": "connecting" }));
            let (sh, a, w) = (sh.clone(), account.clone(), watch.clone());
            std::thread::spawn(move || watch_inbox(&sh, &a, &w));
            r.watches.insert(account.id.clone(), (account, watch));
        }
    }
    let mut widgets = sh.widgets.lock().unwrap();
    gone.iter().fold(false, |changed, id| widgets.remove(&widget_id(id)) || changed)
}

pub fn stop_all(sh: &Shared) {
    for (_, (_, watch)) in sh.mail.lock().unwrap().watches.drain() {
        watch.stop();
    }
}

fn watch_inbox(sh: &Arc<Shared>, account: &Account, watch: &Watch) {
    let mut failures = 0u32;
    // The newest unread letter seen: one newer is new mail.
    let mut newest_seen: Option<u32> = None;
    while !watch.stopped() {
        let result = (|| -> Result<(), Fail> {
            let password = secret::read(&account.id).ok_or_else(|| Fail::Login("no password kept".into()))?;
            let mut conn = session(&account.server, &password)?;
            *watch.socket.lock().unwrap() = conn.socket.try_clone().ok();
            if watch.stopped() {
                return Ok(());
            }
            failures = 0;
            loop {
                let unseen = conn.unseen()?;
                let newest = unseen.iter().max().copied();
                let letter = match newest {
                    Some(uid) => Some(conn.header(uid)?),
                    None => None,
                };
                let is_new = matches!((newest, newest_seen), (Some(n), Some(seen)) if n > seen);
                if newest > newest_seen || newest_seen.is_none() {
                    newest_seen = newest.or(Some(0));
                }
                show(sh, account, Some(unseen.len()), letter.as_ref(), is_new);
                watch.set(json!({ "state": "ok", "unread": unseen.len() }));
                if watch.stopped() {
                    return Ok(());
                }
                if conn.has("IDLE") {
                    conn.idle()?;
                } else {
                    watch.rest(POLL);
                    if watch.stopped() {
                        return Ok(());
                    }
                    conn.command("NOOP")?;
                }
            }
        })();
        watch.socket.lock().unwrap().take();
        let Err(fail) = result else { break };
        if watch.stopped() {
            break;
        }
        failures += 1;
        sh.log(&format!("mail {}: {:?} ({} in a row)", account.id, fail, failures));
        watch.set(json!({ "state": "error", "error": fail.json() }));
        if failures >= 2 {
            show(sh, account, None, None, false);
        }
        // A password said no: not again soon, or the account may be locked.
        let wait = if matches!(fail, Fail::Login(_)) { 300 } else { [15, 30, 60, 120, 300][(failures as usize - 1).min(4)] };
        watch.rest(Duration::from_secs(wait));
    }
}

// The account's widget: the newest unread letter (private: who and what
// about) and how many; "can't get mail" when it keeps failing. A new
// letter opens the island.
fn show(sh: &Shared, account: &Account, unread: Option<usize>, letter: Option<&(String, String)>, is_new: bool) {
    let zh = sh.lang() == "zh";
    let label = match letter {
        Some((from, subject)) if !subject.is_empty() => format!("{}{}{}", if from.is_empty() { &account.address } else { from }, if zh { "：" } else { ": " }, subject),
        Some((from, _)) if !from.is_empty() => from.clone(),
        _ => account.address.clone(),
    };
    let (value, color) = match unread {
        Some(0) => (json!({ "key": "mail.none" }), "#8e8e93"),
        Some(n) => (json!({ "key": "mail.unread", "vars": { "n": n.to_string() } }), "#5e9bff"),
        None => (json!({ "key": "mail.cannot" }), "#ff5c6c"),
    };
    let id = widget_id(&account.id);
    if sh.widgets.lock().unwrap().put_owned(&id, "mail", color, json!(label.chars().take(60).collect::<String>()), value, true, now_ms()) {
        sh.redraw();
    }
    if is_new {
        let words = i18n::t(sh.lang(), "mail.new").replace("{what}", &label);
        sh.nudge_widget(&id, &words.chars().take(60).collect::<String>());
    }
}

// Each account for the settings, with how its watch is going.
pub fn view(sh: &Shared) -> Value {
    let all = accounts(sh);
    let r = sh.mail.lock().unwrap();
    Value::Array(
        all.iter()
            .map(|a| {
                let mut v = a.json();
                v["status"] = r.watches.get(&a.id).map_or(Value::Null, |(_, w)| w.status.lock().unwrap().clone());
                v
            })
            .collect(),
    )
}

// --- From the settings ---------------------------------------------------------------------------

fn new_id() -> String {
    format!("{:08x}", (now_ms() ^ ((std::process::id() as u64) << 20)) & 0xffff_ffff)
}

fn keep_accounts(sh: &Arc<Shared>, all: &[Account]) {
    sh.change(json!({ "mail": all.iter().map(Account::json).collect::<Vec<_>>() }));
    if sync(sh) {
        sh.redraw();
    }
}

#[tauri::command]
pub async fn mail_discover(address: String) -> Value {
    tauri::async_runtime::spawn_blocking(move || discover(&address)).await.unwrap_or_else(|e| json!({ "ok": false, "error": e.to_string() }))
}

// An account added or changed: signed in once to see that it works, then
// kept, its password in the Credential Manager. An empty password keeps the
// one kept before.
#[tauri::command]
pub async fn mail_save(app: AppHandle, account: Value, password: String) -> Value {
    let sh = shared(&app);
    let address = account["address"].as_str().unwrap_or("").trim().to_string();
    let Some(server) = Server::from_json(&account).filter(|_| split(&address).is_some()) else {
        return json!({ "ok": false, "error": { "kind": "fields", "text": "" } });
    };
    let mut all = accounts(&sh);
    let id = account["id"].as_str().filter(|id| all.iter().any(|a| a.id == *id)).map_or_else(new_id, String::from);
    let password = if password.is_empty() { secret::read(&id).unwrap_or_default() } else { password };
    if password.is_empty() {
        return json!({ "ok": false, "error": { "kind": "password", "text": "" } });
    }
    let (s, p) = (server.clone(), password.clone());
    let tried = tauri::async_runtime::spawn_blocking(move || test(&s, &p)).await.unwrap_or_else(|e| Err(Fail::Other(e.to_string())));
    let unread = match tried {
        Ok(n) => n,
        Err(fail) => return json!({ "ok": false, "error": fail.json() }),
    };
    if let Err(e) = secret::write(&id, &password) {
        return json!({ "ok": false, "error": { "kind": "keep", "text": e } });
    }
    let kept = Account { id: id.clone(), address, server, on: true };
    match all.iter_mut().find(|a| a.id == id) {
        Some(a) => *a = kept,
        None => all.push(kept),
    }
    keep_accounts(&sh, &all);
    json!({ "ok": true, "unread": unread, "snapshot": settings::snapshot(&sh) })
}

#[tauri::command]
pub async fn mail_remove(app: AppHandle, id: String) -> Value {
    let sh = shared(&app);
    let all: Vec<Account> = accounts(&sh).into_iter().filter(|a| a.id != id).collect();
    secret::delete(&id);
    keep_accounts(&sh, &all);
    settings::snapshot(&sh)
}

#[tauri::command]
pub async fn mail_switch(app: AppHandle, id: String, on: bool) -> Value {
    let sh = shared(&app);
    let mut all = accounts(&sh);
    if let Some(a) = all.iter_mut().find(|a| a.id == id) {
        a.on = on;
    }
    keep_accounts(&sh, &all);
    settings::snapshot(&sh)
}

// --- The domain's mail hosts (MX), from Windows' resolver ----------------------------------------

#[cfg(windows)]
fn mx_hosts(domain: &str) -> Vec<String> {
    #[repr(C)]
    struct MxData {
        exchange: *const u16,
        preference: u16,
        pad: u16,
    }

    #[repr(C)]
    struct Record {
        next: *mut Record,
        name: *const u16,
        kind: u16,
        data_length: u16,
        flags: u32,
        ttl: u32,
        reserved: u32,
        mx: MxData,
    }

    #[link(name = "dnsapi")]
    extern "system" {
        fn DnsQuery_W(name: *const u16, kind: u16, options: u32, extra: *mut u8, results: *mut *mut Record, reserved: *mut u8) -> i32;
        fn DnsFree(data: *mut Record, free_type: i32);
    }
    const MX: u16 = 15;
    const FREE_RECORD_LIST: i32 = 1;

    let name: Vec<u16> = domain.encode_utf16().chain([0]).collect();
    let mut list: *mut Record = std::ptr::null_mut();
    let mut found: Vec<(u16, String)> = Vec::new();
    // SAFETY: a name ending in 0; the records Windows gives are walked while
    // they are its, and freed once.
    unsafe {
        if DnsQuery_W(name.as_ptr(), MX, 0, std::ptr::null_mut(), &mut list, std::ptr::null_mut()) != 0 || list.is_null() {
            return Vec::new();
        }
        let mut at = list;
        while !at.is_null() {
            if (*at).kind == MX && !(*at).mx.exchange.is_null() {
                let p = (*at).mx.exchange;
                let len = (0..).take_while(|&i| *p.add(i) != 0).count();
                found.push(((*at).mx.preference, String::from_utf16_lossy(std::slice::from_raw_parts(p, len))));
            }
            at = (*at).next;
        }
        DnsFree(list, FREE_RECORD_LIST);
    }
    found.sort();
    found.into_iter().map(|(_, host)| host.to_ascii_lowercase()).collect()
}

#[cfg(not(windows))]
fn mx_hosts(_domain: &str) -> Vec<String> {
    Vec::new()
}

// --- Passwords, in Windows' Credential Manager ----------------------------------------------------

#[cfg(windows)]
mod secret {
    #[repr(C)]
    pub struct Credential {
        flags: u32,
        kind: u32,
        target: *const u16,
        comment: *const u16,
        last_written: [u32; 2],
        blob_size: u32,
        blob: *const u8,
        persist: u32,
        attribute_count: u32,
        attributes: *const u8,
        target_alias: *const u16,
        user_name: *const u16,
    }

    #[link(name = "advapi32")]
    extern "system" {
        fn CredWriteW(credential: *const Credential, flags: u32) -> i32;
        fn CredReadW(target: *const u16, kind: u32, flags: u32, credential: *mut *mut Credential) -> i32;
        fn CredDeleteW(target: *const u16, kind: u32, flags: u32) -> i32;
        fn CredFree(buffer: *mut Credential);
    }
    const GENERIC: u32 = 1;
    const PERSIST_LOCAL_MACHINE: u32 = 2;

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain([0]).collect()
    }

    fn target(id: &str) -> Vec<u16> {
        wide(&format!("Wakuwaku mail {id}"))
    }

    pub fn write(id: &str, password: &str) -> Result<(), String> {
        let (target, user) = (target(id), wide("wakuwaku"));
        let blob = password.as_bytes();
        let credential = Credential {
            flags: 0,
            kind: GENERIC,
            target: target.as_ptr(),
            comment: std::ptr::null(),
            last_written: [0, 0],
            blob_size: blob.len() as u32,
            blob: blob.as_ptr(),
            persist: PERSIST_LOCAL_MACHINE,
            attribute_count: 0,
            attributes: std::ptr::null(),
            target_alias: std::ptr::null(),
            user_name: user.as_ptr(),
        };
        // SAFETY: every pointer is to a buffer alive for the call.
        if unsafe { CredWriteW(&credential, 0) } != 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error().to_string())
        }
    }

    pub fn read(id: &str) -> Option<String> {
        let target = target(id);
        let mut found: *mut Credential = std::ptr::null_mut();
        // SAFETY: the credential Windows gives is read while it is its, then freed.
        unsafe {
            if CredReadW(target.as_ptr(), GENERIC, 0, &mut found) == 0 || found.is_null() {
                return None;
            }
            let bytes = std::slice::from_raw_parts((*found).blob, (*found).blob_size as usize).to_vec();
            CredFree(found);
            String::from_utf8(bytes).ok()
        }
    }

    pub fn delete(id: &str) {
        let target = target(id);
        // SAFETY: a name ending in 0.
        unsafe { CredDeleteW(target.as_ptr(), GENERIC, 0) };
    }

    #[cfg(test)]
    pub fn credential_size() -> usize {
        std::mem::size_of::<Credential>()
    }
}

#[cfg(not(windows))]
mod secret {
    pub fn write(_id: &str, _password: &str) -> Result<(), String> {
        Err("not on this system".into())
    }
    pub fn read(_id: &str) -> Option<String> {
        None
    }
    pub fn delete(_id: &str) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    const QQ: &str = r#"<clientConfig version="1.1"><emailProvider id="qq.com">
      <incomingServer type="pop3"><hostname>pop.qq.com</hostname><port>995</port><socketType>SSL</socketType></incomingServer>
      <incomingServer type="imap"><hostname>imap.qq.com</hostname><port>993</port><socketType>SSL</socketType>
        <username>%EMAILADDRESS%</username><authentication>password-cleartext</authentication></incomingServer>
    </emailProvider></clientConfig>"#;

    #[test]
    fn a_config_gives_its_imap_server_that_takes_a_password() {
        let server = from_config(QQ, "Me@qq.com").unwrap().unwrap();
        assert_eq!(server, Server { host: "imap.qq.com".into(), port: 993, security: Security::Ssl, username: "Me@qq.com".into() });
        let local = QQ.replace("%EMAILADDRESS%", "%EMAILLOCALPART%");
        assert_eq!(from_config(&local, "me@qq.com").unwrap().unwrap().username, "me");
        let oauth = QQ.replace("password-cleartext", "OAuth2");
        assert_eq!(from_config(&oauth, "me@qq.com"), Some(Err(())));
        let both = QQ.replace("<authentication>password-cleartext</authentication>", "<authentication>OAuth2</authentication><authentication>password-cleartext</authentication>");
        assert!(matches!(from_config(&both, "me@qq.com"), Some(Ok(_))));
        assert_eq!(from_config("<clientConfig/>", "me@qq.com"), None);
    }

    #[test]
    fn mail_hosts_belong_to_their_domains() {
        assert_eq!(base_domain("mx1.qq.com"), "qq.com");
        assert_eq!(base_domain("a3011.mx.srv.dfn.de."), "dfn.de");
        assert_eq!(base_domain("mx.mail.sina.com.cn"), "sina.com.cn");
        assert_eq!(base_domain("mail.ox.ac.uk"), "ox.ac.uk");
    }

    #[test]
    fn imap_lines_read_as_they_come() {
        assert_eq!(literal_len(b"* 1 FETCH (UID 7 BODY[HEADER.FIELDS (FROM SUBJECT)] {42}"), Some(42));
        assert_eq!(literal_len(b"* 1 FETCH (FLAGS ())"), None);
        assert_eq!(quoted(r#"pa"ss\word"#), r#""pa\"ss\\word""#);
        let head = "From: =?UTF-8?B?546L5oC7?= <boss@example.com>\r\nSubject: =?GBK?B?1tzO5bXEt72wuA==?=\r\n\r\n";
        assert_eq!(from_and_subject(head.as_bytes()), ("王总".into(), "周五的方案".into()));
        assert_eq!(from_and_subject(b"From: boss@example.com\r\n\r\n").0, "boss@example.com");
    }

    #[test]
    fn what_the_page_sends_is_checked() {
        let ok = json!({ "host": "IMAP.Example.com ", "port": "993", "security": "ssl", "username": "me@example.com" });
        assert_eq!(Server::from_json(&ok).map(|s| (s.host, s.port)), Some(("imap.example.com".into(), 993)));
        assert!(Server::from_json(&json!({ "host": "a b", "port": 993, "username": "x" })).is_none());
        assert!(Server::from_json(&json!({ "host": "a.b", "port": 0, "username": "x" })).is_none());
        assert_eq!(split("x@TU-Dresden.de").map(|(_, d)| d), Some("tu-dresden.de".into()));
        assert!(split("no-at-sign").is_none());
    }

    #[cfg(windows)]
    #[test]
    fn a_password_is_kept_and_forgotten() {
        assert_eq!(secret::credential_size(), if cfg!(target_pointer_width = "64") { 80 } else { 52 });
        let id = format!("test-{}", std::process::id());
        secret::write(&id, "授权码 x\"y").unwrap();
        assert_eq!(secret::read(&id).as_deref(), Some("授权码 x\"y"));
        secret::delete(&id);
        assert_eq!(secret::read(&id), None);
    }
}
