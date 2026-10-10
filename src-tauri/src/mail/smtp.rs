// SMTP, the little of it a mail program sends with: connected as the
// account's outgoing server says (SSL/TLS at once, or STARTTLS), signed in
// (AUTH PLAIN or LOGIN, Thunderbird's "Normal password"), a letter handed
// over (MAIL FROM, RCPT TO for each, DATA), gone. Written here, not taken
// from a library: a few lines each way, over the connection IMAP uses
// (imap.rs).
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use base64::Engine;

use super::imap::{connect, io_fail, tls, Fail, Stream};
use super::{Auth, Security, Server};

// How long the server may take over a letter once it has it all (it may
// look it over first).
const DATA_WAIT: Duration = Duration::from_secs(120);
// The most a line of the server's may be, and lines of a reply.
const LINE_MOST: usize = 8 * 1024;
const LINES_MOST: usize = 200;
// Who we say we are: no machine's name, as Thunderbird says it when it
// keeps that to itself.
const HELLO: &str = "[127.0.0.1]";

// A reply: its code and its lines' words.
#[derive(Debug)]
struct Reply {
    code: u16,
    lines: Vec<String>,
}

impl Reply {
    fn text(&self) -> String {
        self.lines.join(" ").trim().to_string()
    }
}

pub struct Smtp {
    stream: Stream,
    // The same socket, for its read timeout.
    socket: TcpStream,
    // What came and is not yet read as a line.
    got: Vec<u8>,
    // What it can do, as EHLO said: a keyword and its parameters a line.
    exts: Vec<String>,
}

impl Smtp {
    // Connected, encrypted as asked, greeted, and what it can do.
    pub fn open(server: &Server, timeout: Duration) -> Result<Smtp, Fail> {
        let (stream, socket) = connect(server, timeout)?;
        let mut s = Smtp { stream, socket, got: Vec::new(), exts: Vec::new() };
        let hello = s.reply()?;
        if hello.code != 220 {
            return Err(Fail::Refused(hello.text()));
        }
        s.ehlo()?;
        if server.security == Security::StartTls {
            if !s.has("STARTTLS") {
                return Err(Fail::NoStartTls);
            }
            let r = s.command("STARTTLS")?;
            if r.code != 220 {
                return Err(Fail::Tls(r.text()));
            }
            // Bytes the server sent after its yes, before the handshake,
            // would be someone's in the middle: no.
            if !s.got.is_empty() {
                return Err(Fail::Tls("data before the TLS handshake".into()));
            }
            let Stream::Plain(tcp) = std::mem::replace(&mut s.stream, Stream::Gone) else { return Err(Fail::Other("already encrypted".into())) };
            s.stream = tls(&server.host, tcp)?;
            s.ehlo()?;
        }
        Ok(s)
    }

    fn line(&mut self) -> Result<String, Fail> {
        loop {
            if let Some(at) = self.got.iter().position(|&b| b == b'\n') {
                let line: Vec<u8> = self.got.drain(..=at).collect();
                return Ok(String::from_utf8_lossy(&line).trim_end_matches(['\r', '\n']).to_string());
            }
            if self.got.len() > LINE_MOST {
                return Err(Fail::NotSmtp("a line too long".into()));
            }
            let mut buf = [0u8; 4096];
            match self.stream.read(&mut buf).map_err(io_fail)? {
                0 => return Err(Fail::Other("the server closed the connection".into())),
                n => self.got.extend_from_slice(&buf[..n]),
            }
        }
    }

    // A reply: "250-…" lines until a "250 …" one. Anything else (an IMAP
    // server's "* OK") is not SMTP.
    fn reply(&mut self) -> Result<Reply, Fail> {
        let mut lines = Vec::new();
        loop {
            let line = self.line()?;
            let b = line.as_bytes();
            let code = (b.len() >= 3 && b[..3].iter().all(u8::is_ascii_digit) && matches!(b.get(3), None | Some(b' ' | b'-'))).then(|| line[..3].parse::<u16>().ok()).flatten();
            let Some(code) = code.filter(|c| (200..600).contains(c)) else { return Err(Fail::NotSmtp(line.chars().take(80).collect())) };
            lines.push(line.get(4..).unwrap_or("").to_string());
            if b.get(3) != Some(&b'-') {
                return Ok(Reply { code, lines });
            }
            if lines.len() > LINES_MOST {
                return Err(Fail::NotSmtp("a reply too long".into()));
            }
        }
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), Fail> {
        self.stream.write_all(bytes).and_then(|_| self.stream.flush()).map_err(io_fail)
    }

    fn command(&mut self, line: &str) -> Result<Reply, Fail> {
        self.write(format!("{line}\r\n").as_bytes())?;
        self.reply()
    }

    // Hello, and what it can do; HELO for a server from before ESMTP.
    fn ehlo(&mut self) -> Result<(), Fail> {
        let r = self.command(&format!("EHLO {HELLO}"))?;
        if r.code == 250 {
            self.exts = r.lines.iter().skip(1).map(|l| l.trim().to_ascii_uppercase()).collect();
            return Ok(());
        }
        let r = self.command(&format!("HELO {HELLO}"))?;
        if r.code != 250 {
            return Err(Fail::Refused(r.text()));
        }
        self.exts.clear();
        Ok(())
    }

    pub fn has(&self, ext: &str) -> bool {
        self.exts.iter().any(|e| e.split([' ', '=']).next().is_some_and(|k| k.eq_ignore_ascii_case(ext)))
    }

    // How it lets one sign in: AUTH's mechanisms (PLAIN, LOGIN…; "AUTH=…"
    // from older servers too).
    pub fn auths(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for e in &self.exts {
            let Some(rest) = e.strip_prefix("AUTH ").or_else(|| e.strip_prefix("AUTH=")) else { continue };
            for m in rest.split_whitespace() {
                if !out.iter().any(|x| x == m) {
                    out.push(m.to_string());
                }
            }
        }
        out
    }

    // Signed in as `auth` says: AUTH PLAIN, AUTH LOGIN, or (auto) PLAIN
    // where it is offered, else LOGIN; not at all where no way is offered
    // (a server that lets anyone send).
    pub fn login(&mut self, user: &str, password: &str, auth: Auth) -> Result<(), Fail> {
        let offered = self.auths();
        let offers = |m: &str| offered.iter().any(|x| x == m);
        let plain = match auth {
            Auth::Plain => true,
            Auth::Login => false,
            Auth::Auto if offered.is_empty() => return Ok(()),
            Auth::Auto if offers("PLAIN") => true,
            Auth::Auto if offers("LOGIN") => false,
            Auth::Auto => return Err(Fail::Login(format!("AUTH {}", offered.join(" ")))),
        };
        let b64 = |s: &[u8]| base64::engine::general_purpose::STANDARD.encode(s);
        let r = if plain {
            self.command(&format!("AUTH PLAIN {}", b64(format!("\0{user}\0{password}").as_bytes())))?
        } else {
            let r = self.command("AUTH LOGIN")?;
            if r.code != 334 {
                return Err(Fail::Login(r.text()));
            }
            let r = self.command(&b64(user.as_bytes()))?;
            if r.code != 334 {
                return Err(Fail::Login(r.text()));
            }
            self.command(&b64(password.as_bytes()))?
        };
        match r.code {
            235 => Ok(()),
            // Asked for more than the mechanism has: given up.
            334 => {
                let _ = self.command("*");
                Err(Fail::Login(r.text()))
            }
            _ => Err(Fail::Login(r.text())),
        }
    }

    // A letter handed over: from whom, to whom (each), the letter as it
    // goes. Every one it is to must be taken, or it goes to none.
    pub fn send(&mut self, from: &str, to: &[String], letter: &[u8]) -> Result<(), Fail> {
        let wide = !from.is_ascii() || to.iter().any(|t| !t.is_ascii());
        if wide && !self.has("SMTPUTF8") {
            let who = std::iter::once(from).chain(to.iter().map(String::as_str)).find(|a| !a.is_ascii()).unwrap_or("");
            return Err(Fail::Recipient(format!("{who}: SMTPUTF8")));
        }
        let mut mail = format!("MAIL FROM:<{from}>");
        if self.has("SIZE") {
            mail += &format!(" SIZE={}", letter.len());
        }
        if !letter.is_ascii() && self.has("8BITMIME") {
            mail += " BODY=8BITMIME";
        }
        if wide {
            mail += " SMTPUTF8";
        }
        let r = self.command(&mail)?;
        if r.code != 250 {
            return Err(Fail::Refused(r.text()));
        }
        for rcpt in to {
            let r = self.command(&format!("RCPT TO:<{rcpt}>"))?;
            if r.code / 100 != 2 {
                let _ = self.command("RSET");
                return Err(Fail::Recipient(format!("{rcpt}: {}", r.text())));
            }
        }
        let r = self.command("DATA")?;
        if r.code != 354 {
            let _ = self.command("RSET");
            return Err(Fail::Refused(r.text()));
        }
        self.write(&dot_stuffed(letter))?;
        let _ = self.socket.set_read_timeout(Some(DATA_WAIT));
        let r = self.reply()?;
        if r.code != 250 {
            return Err(Fail::Refused(r.text()));
        }
        Ok(())
    }

    pub fn quit(mut self) {
        let _ = self.command("QUIT");
    }
}

// A letter as DATA sends it: every line ending in CRLF, one that starts
// with a dot given another, a dot alone on the last line.
fn dot_stuffed(letter: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(letter.len() + letter.len() / 40 + 8);
    let (mut at_start, mut before) = (true, 0u8);
    for &b in letter {
        if b == b'\n' && before != b'\r' {
            out.push(b'\r');
        }
        if at_start && b == b'.' {
            out.push(b'.');
        }
        out.push(b);
        at_start = b == b'\n';
        before = b;
    }
    if !out.is_empty() && !out.ends_with(b"\r\n") {
        out.extend_from_slice(b"\r\n");
    }
    out.extend_from_slice(b".\r\n");
    out
}

// Connected and signed in.
pub fn session(server: &Server, password: &str, timeout: Duration) -> Result<Smtp, Fail> {
    let mut s = Smtp::open(server, timeout)?;
    s.login(&server.username, password, server.auth)?;
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};
    use std::net::TcpListener;

    #[test]
    fn a_letter_is_stuffed_for_data() {
        assert_eq!(dot_stuffed(b"a\n.b\r\n..c"), b"a\r\n..b\r\n...c\r\n.\r\n");
        assert_eq!(dot_stuffed(b".\r\n"), b"..\r\n.\r\n");
    }

    // A server that says what a script says, line by line, and keeps what it heard.
    fn server(script: &'static [(&'static str, &'static str)]) -> (u16, std::thread::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let heard = std::thread::spawn(move || {
            let (tcp, _) = listener.accept().unwrap();
            let mut out = tcp.try_clone().unwrap();
            let mut lines = BufReader::new(tcp).lines();
            out.write_all(b"220 fake ESMTP\r\n").unwrap();
            let mut heard = Vec::new();
            for (expect, say) in script {
                let mut line = lines.next().unwrap().unwrap();
                if *expect == "<data>" {
                    while line != "." {
                        heard.push(line);
                        line = lines.next().unwrap().unwrap();
                    }
                } else {
                    assert!(line.starts_with(expect), "{line} is not {expect}");
                }
                heard.push(line);
                out.write_all(say.as_bytes()).unwrap();
            }
            heard
        });
        (port, heard)
    }

    #[test]
    fn a_letter_goes_through() {
        let (port, heard) = server(&[
            ("EHLO [127.0.0.1]", "250-fake\r\n250-SIZE 1000\r\n250 AUTH LOGIN PLAIN\r\n"),
            ("AUTH PLAIN AG1lAHB3", "235 ok\r\n"),
            ("MAIL FROM:<me@a.test> SIZE=", "250 ok\r\n"),
            ("RCPT TO:<you@b.test>", "250 ok\r\n"),
            ("RCPT TO:<them@c.test>", "251 will forward\r\n"),
            ("DATA", "354 go on\r\n"),
            ("<data>", "250 queued\r\n"),
            ("QUIT", "221 bye\r\n"),
        ]);
        let server = Server { host: "127.0.0.1".into(), port, security: Security::Plain, username: "me".into(), auth: Auth::Auto };
        let mut s = session(&server, "pw", Duration::from_secs(5)).unwrap();
        assert_eq!(s.auths(), ["LOGIN", "PLAIN"]);
        s.send("me@a.test", &["you@b.test".into(), "them@c.test".into()], b"Subject: hi\r\n\r\n.dot\r\n").unwrap();
        s.quit();
        let heard = heard.join().unwrap();
        assert!(heard.contains(&"..dot".to_string()), "{heard:?}");
    }

    #[test]
    fn a_refused_one_stops_it_all() {
        let (port, heard) = server(&[
            ("EHLO", "250-fake\r\n250 AUTH LOGIN\r\n"),
            ("AUTH LOGIN", "334 VXNlcm5hbWU6\r\n"),
            ("bWU=", "334 UGFzc3dvcmQ6\r\n"),
            ("cHc=", "235 ok\r\n"),
            ("MAIL FROM:<me@a.test>", "250 ok\r\n"),
            ("RCPT TO:<nobody@b.test>", "550 5.1.1 no such user\r\n"),
            ("RSET", "250 ok\r\n"),
        ]);
        let server = Server { host: "127.0.0.1".into(), port, security: Security::Plain, username: "me".into(), auth: Auth::Auto };
        let mut s = session(&server, "pw", Duration::from_secs(5)).unwrap();
        assert_eq!(s.send("me@a.test", &["nobody@b.test".into()], b"x\r\n"), Err(Fail::Recipient("nobody@b.test: 5.1.1 no such user".into())));
        drop(s);
        heard.join().unwrap();
    }

    #[test]
    fn a_password_said_no_to_says_why() {
        let (port, heard) = server(&[("EHLO", "250-fake\r\n250 AUTH PLAIN\r\n"), ("AUTH PLAIN", "535 5.7.8 Username and Password not accepted\r\n")]);
        let server = Server { host: "127.0.0.1".into(), port, security: Security::Plain, username: "me".into(), auth: Auth::Auto };
        assert_eq!(session(&server, "pw", Duration::from_secs(5)).err(), Some(Fail::Login("5.7.8 Username and Password not accepted".into())));
        heard.join().unwrap();
    }
}
