// How mail reaches its servers: through Windows' proxy (the default, and
// straight when none is set), through one typed in, or straight. The Mail
// page sets it for every account ("mailProxy"), and an account may have its
// own ("proxy" in it; null follows the page): { mode: system | manual |
// none, kind: http | socks5, address: "host:port" }. IMAP (its IDLE too),
// SMTP and the lookups (the autoconfig files over HTTPS, the guesses over
// IMAP and SMTP) all go the same way; the domain's MX records are asked of
// Windows as before (no proxy carries DNS).
//
// The way through a proxy is made here, a few lines each way as smtp.rs is:
// HTTP's CONNECT host:port, its 200, then the server's own bytes (TLS, or
// its greeting); SOCKS5 with the server's name (ATYP 3), so the proxy looks
// it up and not the DNS here (which may be poisoned). What goes wrong says
// which hop it was: the proxy not reached, not answering as a proxy of that
// kind, saying no (an HTTP one that lets CONNECT through to 443 alone: the
// page says to try SOCKS5), or not reaching the server.
use std::io::{self, Read, Write};
use std::net::{IpAddr, TcpStream, ToSocketAddrs};
use std::time::Duration;

use serde_json::{json, Value};

use super::imap::{Fail, Hop};
pub use crate::proxy::{Kind, Proxy, Via};
use crate::Shared;

// The way set on the Mail page: the General page's Network proxy (netProxy,
// Windows' unless set) while it follows that ("follow", and unset: since
// zhoupengjie/Wakuwaku#68; one set before keeps its way), else its own.
pub fn setting(sh: &Shared) -> Via {
    let page = sh.config().mail_proxy.clone();
    if follows(&page) {
        return Via::of(&sh.config().net_proxy).unwrap_or_default();
    }
    Via::of(&page).unwrap_or_default()
}

fn follows(page: &Value) -> bool {
    page.is_null() || page["mode"] == "follow"
}

// What the Mail page may keep: a way, or "follow" (a manual one's kind and
// address kept beside it, as with the others).
pub fn is_page_way(v: &Value) -> bool {
    if v.get("mode").and_then(Value::as_str) != Some("follow") {
        return Via::of(v).is_some();
    }
    v.as_object().is_some_and(|m| m.iter().all(|(k, x)| match k.as_str() {
        "mode" => true,
        "kind" => x.as_str().and_then(Kind::parse).is_some(),
        "address" => x.is_string(),
        _ => false,
    }))
}

// The way for one asked from the page: its own, or (null) the page's.
pub fn chosen(sh: &Shared, own: &Value) -> Option<Via> {
    if own.is_null() {
        Some(setting(sh))
    } else {
        Via::of(own)
    }
}

// The proxy on the way to a host: Windows' (unless its list says to reach
// that host straight), the one typed in, or none.
pub fn route(via: &Via, host: &str) -> Option<Proxy> {
    match via {
        Via::System => crate::proxy::system().filter(|s| !s.bypasses(host)).and_then(|s| s.for_mail()),
        Via::Manual(p) => Some(p.clone()),
        Via::Straight => None,
    }
}

// Windows' proxy as mail would take it, for the page: { kind, address }, or null.
pub fn system_view() -> Value {
    crate::proxy::system().and_then(|s| s.for_mail()).map_or(Value::Null, |p| json!({ "kind": p.kind.name(), "address": p.at() }))
}

// What a SOCKS5 proxy says when it does not connect (RFC 1928): its words
// for the codes that mean the server was not reached.
const SOCKS_WORDS: [&str; 9] = ["", "general failure", "not allowed by its rules", "network unreachable", "host unreachable", "connection refused", "TTL expired", "command not supported", "address type not supported"];

// A host and port as a request line writes them.
fn target(host: &str, port: u16) -> String {
    if host.contains(':') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    }
}

// A connection to the proxy itself.
pub fn reach(p: &Proxy, timeout: Duration) -> Result<TcpStream, Fail> {
    let down = |why: String| Fail::ProxyDown(Hop { at: p.at(), why, hint: "" });
    let addr = (p.host.as_str(), p.port).to_socket_addrs().map_err(|e| down(e.to_string()))?.next().ok_or_else(|| down("no address".into()))?;
    TcpStream::connect_timeout(&addr, timeout).map_err(|e| down(e.to_string()))
}

// A connection to host:port through a proxy, the server's own bytes next; or
// which hop failed.
pub fn tunnel(p: &Proxy, host: &str, port: u16, timeout: Duration) -> Result<TcpStream, Fail> {
    let at = p.at();
    let mut tcp = reach(p, timeout)?;
    let _ = tcp.set_read_timeout(Some(timeout));
    let _ = tcp.set_write_timeout(Some(timeout));
    let said = match p.kind {
        Kind::Http => through_http(&mut tcp, host, port),
        Kind::Socks5 => through_socks(&mut tcp, host, port),
    };
    said.map_err(|(fail, why, hint): Said| fail(Hop { at, why, hint }))?;
    Ok(tcp)
}

// What went wrong on the way: the kind of failure, its words, a hint.
type Said = (fn(Hop) -> Fail, String, &'static str);

// A read that timed out: the proxy is still trying (an HTTP one answers
// CONNECT once it has reached the server), or is not one that answers this.
fn waited(e: &io::Error) -> bool {
    matches!(e.kind(), io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock)
}

// CONNECT host:port; its answer read a byte at a time to the blank line, as
// what comes after it is the server's (an IMAP greeting may come with it).
fn through_http(tcp: &mut TcpStream, host: &str, port: u16) -> Result<(), Said> {
    let to = target(host, port);
    let ask = format!("CONNECT {to} HTTP/1.1\r\nHost: {to}\r\nUser-Agent: wakuwaku\r\nProxy-Connection: keep-alive\r\n\r\n");
    tcp.write_all(ask.as_bytes()).map_err(|e| (Fail::ProxyDown as fn(Hop) -> Fail, e.to_string(), ""))?;
    let mut head = Vec::new();
    while !head.ends_with(b"\r\n\r\n") {
        let mut b = [0u8; 1];
        match tcp.read(&mut b) {
            // Closed at once (reset, on Windows, with our request unread):
            // something there that took "CONNECT" for nothing it speaks (a
            // SOCKS proxy does so).
            Ok(0) => return Err((Fail::NotProxy, "closed the connection".into(), if head.is_empty() { "socks" } else { "" })),
            Err(e) if head.is_empty() && matches!(e.kind(), io::ErrorKind::ConnectionReset | io::ErrorKind::ConnectionAborted) => {
                return Err((Fail::NotProxy, "closed the connection".into(), "socks"));
            }
            // A SOCKS5 proxy's answer, from one that read on regardless.
            Ok(_) if head.is_empty() && b[0] == 5 => return Err((Fail::NotProxy, "answers as a SOCKS5 proxy".into(), "socks")),
            Ok(_) => head.push(b[0]),
            Err(e) if waited(&e) && head.is_empty() => return Err((Fail::ProxyServer, "no answer in time".into(), "")),
            Err(e) => return Err((Fail::NotProxy, e.to_string(), "")),
        }
        if head.len() > 8192 {
            return Err((Fail::NotProxy, "an answer too long".into(), ""));
        }
    }
    let text = String::from_utf8_lossy(&head);
    let status = text.lines().next().unwrap_or("").trim().to_string();
    let mut words = status.splitn(3, ' ');
    let version = words.next().unwrap_or("");
    let Some(code) = words.next().and_then(|c| c.parse::<u16>().ok()).filter(|_| version.starts_with("HTTP/1.")) else {
        return Err((Fail::NotProxy, status.chars().take(80).collect(), "socks"));
    };
    let said: String = words.next().unwrap_or("").chars().take(80).collect();
    let said = format!("{code} {said}").trim().to_string();
    match code {
        200..=299 => Ok(()),
        407 => Err((Fail::ProxyRefused, format!("{said} (it wants a password)"), "")),
        502..=504 => Err((Fail::ProxyServer, said, "")),
        // A proxy that lets CONNECT through to 443 alone (Squid's way): SOCKS5 goes anywhere.
        _ => Err((Fail::ProxyRefused, said, if port == 443 { "" } else { "socks" })),
    }
}

fn read_all(tcp: &mut TcpStream, buf: &mut [u8], waiting: fn(Hop) -> Fail) -> Result<(), Said> {
    tcp.read_exact(buf).map_err(|e| -> Said {
        match e.kind() {
            io::ErrorKind::UnexpectedEof => (Fail::NotProxy, "closed the connection".into(), ""),
            _ if waited(&e) => (waiting, "no answer in time".into(), ""),
            _ => (Fail::NotProxy, e.to_string(), ""),
        }
    })
}

// SOCKS5 (RFC 1928): no password; CONNECT by the server's name (an IP
// address as one), for the proxy to look it up.
fn through_socks(tcp: &mut TcpStream, host: &str, port: u16) -> Result<(), Said> {
    tcp.write_all(&[5, 1, 0]).map_err(|e| (Fail::ProxyDown as fn(Hop) -> Fail, e.to_string(), ""))?;
    let mut hello = [0u8; 2];
    // An HTTP proxy waits for the rest of a request line that never comes, or says 400.
    read_all(tcp, &mut hello, Fail::NotProxy).map_err(|(f, why, _)| (f, why, "http"))?;
    if &hello == b"HT" {
        return Err((Fail::NotProxy, "answers as an HTTP proxy".into(), "http"));
    }
    if hello[0] != 5 {
        return Err((Fail::NotProxy, format!("answers {:02x} {:02x}", hello[0], hello[1]), ""));
    }
    if hello[1] == 0xff {
        return Err((Fail::ProxyRefused, "it wants a password".into(), ""));
    }
    if hello[1] != 0 {
        return Err((Fail::NotProxy, format!("asks for method {}", hello[1]), ""));
    }
    let mut ask = vec![5, 1, 0];
    match host.parse::<IpAddr>() {
        Ok(IpAddr::V4(ip)) => {
            ask.push(1);
            ask.extend(ip.octets());
        }
        Ok(IpAddr::V6(ip)) => {
            ask.push(4);
            ask.extend(ip.octets());
        }
        Err(_) => {
            let name = host.as_bytes();
            if name.len() > 255 {
                return Err((Fail::ProxyRefused, "the server's name is too long".into(), ""));
            }
            ask.push(3);
            ask.push(name.len() as u8);
            ask.extend(name);
        }
    }
    ask.extend(port.to_be_bytes());
    tcp.write_all(&ask).map_err(|e| (Fail::ProxyDown as fn(Hop) -> Fail, e.to_string(), ""))?;
    let mut head = [0u8; 4];
    read_all(tcp, &mut head, Fail::ProxyServer)?;
    if head[0] != 5 {
        return Err((Fail::NotProxy, format!("answers {:02x}", head[0]), ""));
    }
    match head[1] {
        0 => {}
        2 | 7 | 8 => return Err((Fail::ProxyRefused, SOCKS_WORDS[head[1] as usize].into(), "")),
        1 | 3..=6 => return Err((Fail::ProxyServer, SOCKS_WORDS[head[1] as usize].into(), "")),
        n => return Err((Fail::NotProxy, format!("answers {n}"), "")),
    }
    // The address it connected from, which is nothing to us.
    let len = match head[3] {
        1 => 4,
        4 => 16,
        3 => {
            let mut n = [0u8; 1];
            read_all(tcp, &mut n, Fail::NotProxy)?;
            n[0] as usize
        }
        n => return Err((Fail::NotProxy, format!("an address of type {n}"), "")),
    };
    let mut rest = vec![0u8; len + 2];
    read_all(tcp, &mut rest, Fail::NotProxy)
}

// --- The lookups' web requests (discover.rs), through the proxy ------------------------------------

// ureq with our way through the proxy in front of its own TLS, from its
// "unversioned" connectors (not under semver: Cargo.lock holds 3.4.2). The
// proxy put in its config is only for ureq to leave the server's name to
// the proxy, as a socks5h one has it (ureq's own SOCKS is not built in, and
// is never called: ours stands where it would; without it, the name is
// looked up here, and still the connection goes through ours).
pub fn agent(proxy: Option<&Proxy>, timeout: Duration) -> ureq::Agent {
    let config = ureq::Agent::config_builder().timeout_global(Some(timeout));
    match proxy {
        Some(p) => through(p, timeout, config),
        None => config.build().new_agent(),
    }
}

// An agent through `p` with this config (the web's way through one typed
// in, proxy::Way::Manual, too); `connect`: how long the way through may take.
pub fn through(p: &Proxy, connect: Duration, config: ureq::config::ConfigBuilder<ureq::typestate::AgentScope>) -> ureq::Agent {
    use ureq::unversioned::resolver::DefaultResolver;
    use ureq::unversioned::transport::{Connector, RustlsConnector};
    let config = match ureq::Proxy::new(&format!("socks5h://{}", p.at())) {
        Ok(stand_in) => config.proxy(Some(stand_in)),
        Err(_) => config,
    }
    .build();
    let connector = ().chain(Through { proxy: p.clone(), timeout: connect }).chain(RustlsConnector::default());
    ureq::Agent::with_parts(config, connector, DefaultResolver::default())
}

#[derive(Debug)]
struct Through {
    proxy: Proxy,
    timeout: Duration,
}

impl ureq::unversioned::transport::Connector<()> for Through {
    type Out = Tunnel;

    fn connect(&self, details: &ureq::unversioned::transport::ConnectionDetails, _: Option<()>) -> Result<Option<Tunnel>, ureq::Error> {
        let uri = details.uri;
        let host = uri.host().unwrap_or("").trim_start_matches('[').trim_end_matches(']');
        let port = uri.port_u16().unwrap_or(if uri.scheme_str() == Some("https") { 443 } else { 80 });
        let tcp = tunnel(&self.proxy, host, port, self.timeout).map_err(|f| ureq::Error::Io(io::Error::other(hop_words(&f))))?;
        let config = details.config;
        Ok(Some(Tunnel { tcp, buffers: ureq::unversioned::transport::LazyBuffers::new(config.input_buffer_size(), config.output_buffer_size()) }))
    }
}

// A way through that failed, in a few words: the lookups' and the web's
// errors (an update's log, the Network proxy's Test).
fn hop_words(f: &Fail) -> String {
    match f {
        Fail::ProxyDown(h) => format!("proxy {} not reached: {}", h.at, h.why),
        Fail::NotProxy(h) => format!("{} does not answer as a proxy of this kind: {}", h.at, h.why),
        Fail::ProxyRefused(h) => format!("proxy {} said no: {}", h.at, h.why),
        Fail::ProxyServer(h) => format!("proxy {} did not reach the server: {}", h.at, h.why),
        other => format!("{other:?}"),
    }
}

// The connection through the proxy, as ureq reads and writes one (its own
// TcpTransport is not to be had); not kept for another request.
#[derive(Debug)]
struct Tunnel {
    tcp: TcpStream,
    buffers: ureq::unversioned::transport::LazyBuffers,
}

impl Tunnel {
    fn failed(e: io::Error, timeout: &ureq::unversioned::transport::NextTimeout) -> ureq::Error {
        if waited(&e) {
            ureq::Error::Timeout(timeout.reason)
        } else {
            e.into()
        }
    }
}

impl ureq::unversioned::transport::Transport for Tunnel {
    fn buffers(&mut self) -> &mut dyn ureq::unversioned::transport::Buffers {
        &mut self.buffers
    }

    fn transmit_output(&mut self, amount: usize, timeout: ureq::unversioned::transport::NextTimeout) -> Result<(), ureq::Error> {
        use ureq::unversioned::transport::Buffers;
        let _ = self.tcp.set_write_timeout(timeout.not_zero().map(|t| *t));
        let output = &self.buffers.output()[..amount];
        self.tcp.write_all(output).map_err(|e| Tunnel::failed(e, &timeout))
    }

    fn await_input(&mut self, timeout: ureq::unversioned::transport::NextTimeout) -> Result<bool, ureq::Error> {
        use ureq::unversioned::transport::Buffers;
        let _ = self.tcp.set_read_timeout(timeout.not_zero().map(|t| *t));
        let n = self.tcp.read(self.buffers.input_append_buf()).map_err(|e| Tunnel::failed(e, &timeout))?;
        self.buffers.input_appended(n);
        Ok(n > 0)
    }

    fn is_open(&mut self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use crate::lock::Lock;
    use super::*;
    use crate::mail::imap::Session;
    use crate::mail::smtp::Smtp;
    use crate::mail::{Auth, Security, Server};
    use std::io::{BufRead, BufReader};
    use std::net::{Shutdown, TcpListener};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Instant;

    // The name the proxies here take to the server behind them; any other,
    // they cannot reach.
    const NAME: &str = "mail.example.test";
    const WAIT: Duration = Duration::from_secs(5);

    // A server on this computer that says a greeting and answers each line
    // as `answer` says (None: it closes): the mail server behind the proxies.
    fn server(greeting: &'static str, answer: fn(&str) -> Option<String>) -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        thread::spawn(move || {
            for conn in listener.incoming().flatten() {
                thread::spawn(move || {
                    let mut out = conn.try_clone().unwrap();
                    let _ = out.write_all(greeting.as_bytes());
                    for line in BufReader::new(conn).lines().map_while(Result::ok) {
                        match answer(&line) {
                            Some(said) => {
                                let _ = out.write_all(said.as_bytes());
                            }
                            None => return,
                        }
                    }
                });
            }
        });
        port
    }

    // One that echoes each line.
    fn echo() -> u16 {
        server("* OK hello\r\n", |line| Some(format!("{line}\r\n")))
    }

    // Bytes both ways until either side closes.
    fn pipe(a: TcpStream, b: TcpStream) {
        let (mut a2, mut b2) = (a.try_clone().unwrap(), b.try_clone().unwrap());
        let (mut a, mut b) = (a, b);
        thread::spawn(move || {
            let _ = io::copy(&mut a2, &mut b2);
            let _ = b2.shutdown(Shutdown::Both);
        });
        let _ = io::copy(&mut b, &mut a);
        let _ = a.shutdown(Shutdown::Both);
    }

    // A little HTTP proxy: CONNECT to NAME (the server here), 502 for any
    // other; or, as Squid by default, 403 for any port but 443.
    fn http_proxy(server: u16, only_443: bool) -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        thread::spawn(move || {
            for conn in listener.incoming().flatten() {
                thread::spawn(move || {
                    let mut conn = conn;
                    let mut head = Vec::new();
                    let mut b = [0u8; 1];
                    while !head.ends_with(b"\r\n\r\n") && conn.read(&mut b).unwrap_or(0) == 1 {
                        head.push(b[0]);
                    }
                    let text = String::from_utf8_lossy(&head).to_string();
                    let to = text.split(' ').nth(1).unwrap_or("").to_string();
                    let (host, port) = to.rsplit_once(':').unwrap_or(("", ""));
                    if only_443 && port != "443" {
                        let _ = conn.write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n");
                        return;
                    }
                    if host != NAME {
                        let _ = conn.write_all(b"HTTP/1.1 502 Bad Gateway\r\n\r\n");
                        return;
                    }
                    let up = TcpStream::connect(("127.0.0.1", server)).unwrap();
                    // The server's greeting is there before the 200: they may well come together.
                    thread::sleep(Duration::from_millis(50));
                    let _ = conn.write_all(b"HTTP/1.1 200 Connection established\r\n\r\n");
                    pipe(conn, up);
                });
            }
        });
        port
    }

    // What a little SOCKS5 proxy keeps: what it was asked, and the
    // connections it took (to be cut, as a proxy restarted cuts them).
    #[derive(Default, Clone)]
    struct Kept {
        asked: Arc<Mutex<Vec<u8>>>,
        conns: Arc<Mutex<Vec<TcpStream>>>,
    }

    // The proxy: CONNECT by name to NAME, nothing else.
    fn socks_proxy(server: u16, kept: Kept) -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        thread::spawn(move || {
            for conn in listener.incoming().flatten() {
                let kept = kept.clone();
                thread::spawn(move || {
                    let mut conn = conn;
                    // Not SOCKS5 ("CONNECT …"): closed, as a SOCKS5 proxy does.
                    let mut hello = [0u8; 3];
                    if conn.read_exact(&mut hello).is_err() || hello[0] != 5 {
                        return;
                    }
                    conn.write_all(&[5, 0]).unwrap();
                    let mut head = [0u8; 5];
                    conn.read_exact(&mut head).unwrap();
                    let mut name = vec![0u8; head[4] as usize + 2];
                    conn.read_exact(&mut name).unwrap();
                    *kept.asked.lock().unwrap() = head.iter().chain(&name).copied().collect();
                    if head[3] != 3 || &name[..name.len() - 2] != NAME.as_bytes() {
                        conn.write_all(&[5, 4, 0, 1, 0, 0, 0, 0, 0, 0]).unwrap();
                        return;
                    }
                    let up = TcpStream::connect(("127.0.0.1", server)).unwrap();
                    conn.write_all(&[5, 0, 0, 1, 127, 0, 0, 1, 0x1f, 0x90]).unwrap();
                    kept.conns.lock().unwrap().push(conn.try_clone().unwrap());
                    pipe(conn, up);
                });
            }
        });
        port
    }

    fn proxy(kind: Kind, port: u16) -> Proxy {
        Proxy { kind, host: "127.0.0.1".into(), port }
    }

    fn free_port() -> u16 {
        TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
    }

    // The server's greeting, then a line echoed: the tunnel carries both ways.
    fn talks(mut tcp: TcpStream) {
        let mut got = [0u8; 12];
        tcp.read_exact(&mut got).unwrap();
        assert_eq!(&got, b"* OK hello\r\n");
        tcp.write_all(b"a1 NOOP\r\n").unwrap();
        let mut back = [0u8; 9];
        tcp.read_exact(&mut back).unwrap();
        assert_eq!(&back, b"a1 NOOP\r\n");
    }

    #[test]
    fn through_an_http_proxy() {
        let http = http_proxy(echo(), false);
        talks(tunnel(&proxy(Kind::Http, http), NAME, 993, WAIT).unwrap());
        // It could not reach the server: that hop.
        let Err(Fail::ProxyServer(hop)) = tunnel(&proxy(Kind::Http, http), "imap.nowhere.test", 993, WAIT) else { panic!() };
        assert_eq!((hop.at, hop.why.as_str()), (format!("127.0.0.1:{http}"), "502 Bad Gateway"));
        // One that lets CONNECT through to 443 alone: try SOCKS5.
        let squid = http_proxy(echo(), true);
        let Err(Fail::ProxyRefused(hop)) = tunnel(&proxy(Kind::Http, squid), NAME, 993, WAIT) else { panic!() };
        assert_eq!((hop.why.as_str(), hop.hint), ("403 Forbidden", "socks"));
    }

    #[test]
    fn through_a_socks5_proxy_by_name() {
        let kept = Kept::default();
        let socks = socks_proxy(echo(), kept.clone());
        talks(tunnel(&proxy(Kind::Socks5, socks), NAME, 993, WAIT).unwrap());
        // CONNECT, by name (ATYP 3), port 993.
        let mut want = vec![5, 1, 0, 3, NAME.len() as u8];
        want.extend(NAME.as_bytes());
        want.extend([3, 0xe1]);
        assert_eq!(*kept.asked.lock().unwrap(), want);
        let Err(Fail::ProxyServer(hop)) = tunnel(&proxy(Kind::Socks5, socks), "imap.nowhere.test", 993, WAIT) else { panic!() };
        assert_eq!(hop.why, "host unreachable");
    }

    #[test]
    fn each_hop_says_what_went_wrong() {
        // Nothing there.
        let free = free_port();
        let Err(Fail::ProxyDown(hop)) = tunnel(&proxy(Kind::Http, free), NAME, 993, WAIT) else { panic!() };
        assert_eq!(hop.at, format!("127.0.0.1:{free}"));
        assert_eq!(Fail::ProxyDown(hop).json()["proxy"], format!("127.0.0.1:{free}"));
        // The kind mixed up: a SOCKS5 proxy spoken HTTP to closes; an HTTP one
        // spoken SOCKS5 to waits for the rest of a request line, or says 400.
        let socks = socks_proxy(echo(), Kept::default());
        let Err(Fail::NotProxy(hop)) = tunnel(&proxy(Kind::Http, socks), NAME, 993, WAIT) else { panic!() };
        assert_eq!(hop.hint, "socks");
        let http = http_proxy(echo(), false);
        let Err(Fail::NotProxy(hop)) = tunnel(&proxy(Kind::Socks5, http), NAME, 993, Duration::from_millis(500)) else { panic!() };
        assert_eq!(hop.hint, "http");
        // A mail server is no proxy.
        let Err(Fail::NotProxy(hop)) = tunnel(&proxy(Kind::Http, echo()), NAME, 993, WAIT) else { panic!() };
        assert_eq!(hop.why, "* OK hello");
    }

    #[test]
    fn the_way_is_kept_as_set() {
        assert_eq!(Via::of(&json!({ "mode": "system" })), Some(Via::System));
        // The page may follow the General page's (a manual one's kept beside it); an account may not.
        assert!(is_page_way(&json!({ "mode": "follow" })));
        assert!(is_page_way(&json!({ "mode": "follow", "kind": "socks5", "address": "127.0.0.1:10808" })));
        assert!(!is_page_way(&json!({ "mode": "follow", "kind": "ftp" })));
        assert!(!is_page_way(&json!({ "mode": "follow", "other": 1 })));
        assert!(is_page_way(&json!({ "mode": "none" })));
        assert!(follows(&Value::Null) && follows(&json!({ "mode": "follow" })) && !follows(&json!({ "mode": "system" })));
        assert_eq!(Via::of(&json!({ "mode": "none" })), Some(Via::Straight));
        let manual = Via::of(&json!({ "mode": "manual", "kind": "socks5", "address": " 127.0.0.1:10808" })).unwrap();
        assert_eq!(manual, Via::Manual(proxy(Kind::Socks5, 10808)));
        assert_eq!(Via::of(&manual.json()), Some(manual));
        assert_eq!(Via::of(&json!({ "mode": "manual", "address": "127.0.0.1:7890" })), Some(Via::Manual(proxy(Kind::Http, 7890))));
        assert_eq!(Via::of(&json!({ "mode": "manual", "kind": "http", "address": "" })), None);
        assert_eq!(Via::of(&json!({ "mode": "manual", "kind": "ftp", "address": "127.0.0.1:21" })), None);
        assert_eq!(Via::of(&json!({ "mode": "system", "extra": 1 })), None);
        assert_eq!(Via::of(&json!("system")), None);
        assert_eq!(route(&Via::Straight, "imap.gmail.com"), None);
        assert_eq!(route(&Via::Manual(proxy(Kind::Http, 7890)), "127.0.0.1"), Some(proxy(Kind::Http, 7890)));
    }

    fn via(kind: Kind, port: u16, server_port: u16) -> Server {
        Server { host: NAME.into(), port: server_port, security: Security::Plain, username: "me".into(), auth: Auth::Auto, via: Via::Manual(proxy(kind, port)) }
    }

    // SMTP through the HTTP proxy: its greeting, come before the proxy's
    // 200, is not lost.
    #[test]
    fn smtp_goes_through_it() {
        let smtp = server("220 hi\r\n", |line| match line.split(' ').next().unwrap_or("") {
            "EHLO" => Some("250-hi\r\n250 SIZE 1000\r\n".into()),
            "QUIT" => None,
            _ => Some("500 what\r\n".into()),
        });
        let http = http_proxy(smtp, false);
        let s = Smtp::open(&via(Kind::Http, http, 587), WAIT).unwrap();
        assert!(s.has("SIZE"));
        s.quit();
    }

    // The little of an IMAP server an IDLE needs: its greeting (IDLE among
    // what it can do), IDLE and DONE, LOGOUT; the IDLEs counted.
    static IDLES: AtomicUsize = AtomicUsize::new(0);
    static IDLE_TAG: Lock<String> = Lock::new("mail::proxy::IDLE_TAG", String::new());

    fn imap() -> u16 {
        server("* OK [CAPABILITY IMAP4rev1 IDLE] ready\r\n", |line| {
            let tag = line.split(' ').next().unwrap_or("").to_string();
            let command = line.split(' ').nth(1).unwrap_or("").to_ascii_uppercase();
            Some(match command.as_str() {
                _ if tag.eq_ignore_ascii_case("DONE") => format!("{} OK IDLE done\r\n", IDLE_TAG.lock().unwrap()),
                "IDLE" => {
                    IDLES.fetch_add(1, Ordering::SeqCst);
                    *IDLE_TAG.lock().unwrap() = tag;
                    "+ idling\r\n".into()
                }
                "LOGOUT" => format!("* BYE\r\n{tag} OK bye\r\n"),
                _ => format!("{tag} OK\r\n"),
            })
        })
    }

    // Through a proxy an IDLE is renewed now and then, quiet as the server
    // is; and when the proxy goes (restarted), the IDLE ends at once, a
    // failure, for the sync to connect again (through the new one).
    #[test]
    fn imap_idles_through_it_and_sees_it_go() {
        let kept = Kept::default();
        let socks = socks_proxy(imap(), kept.clone());
        let mut s = Session::open(&via(Kind::Socks5, socks, 143), WAIT).unwrap();
        assert_eq!(s.proxy.as_ref().map(Proxy::at), Some(format!("127.0.0.1:{socks}")));
        assert!(s.has("IDLE"));
        s.renew_every(Duration::from_millis(300));
        let kick = Arc::new(AtomicBool::new(false));
        let kicker = kick.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(1100));
            kicker.store(true, Ordering::SeqCst);
        });
        assert_eq!(s.idle(Duration::from_secs(9 * 60), &kick), Ok(false));
        assert!(IDLES.load(Ordering::SeqCst) >= 3, "{} IDLEs", IDLES.load(Ordering::SeqCst));
        kick.store(false, Ordering::SeqCst);
        let cut = kept.conns.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(500));
            for conn in cut.lock().unwrap().drain(..) {
                let _ = conn.shutdown(Shutdown::Both);
            }
        });
        let started = Instant::now();
        assert!(s.idle(Duration::from_secs(9 * 60), &kick).is_err());
        assert!(started.elapsed() < Duration::from_secs(3), "{:?}", started.elapsed());
        Session::open(&via(Kind::Socks5, socks, 143), WAIT).unwrap().logout();
    }

    // An HTTPS lookup through the proxy (the TLS is the server's, end to
    // end): here the server is no web server, so the request fails past the
    // proxy, in the TLS; with the proxy down, before it.
    #[test]
    fn the_lookups_go_through_it_too() {
        let http = http_proxy(echo(), false);
        let err = agent(Some(&proxy(Kind::Http, http)), WAIT).get(&format!("https://{NAME}/mail/config-v1.1.xml")).call().unwrap_err().to_string();
        assert!(!err.contains("proxy"), "{err}");
        let port = free_port();
        let err = agent(Some(&proxy(Kind::Socks5, port)), WAIT).get(&format!("https://{NAME}/")).call().unwrap_err().to_string();
        assert!(err.contains(&format!("proxy 127.0.0.1:{port} not reached: ")), "{err}");
    }
}
