// Finding an address's servers the way Thunderbird does, the IMAP one and
// the outgoing (SMTP) one: the few we know ourselves, the domain's own
// autoconfig, Thunderbird's database ISPDB, the database's entry for the
// provider of the domain's MX host, then a guess where something answers.
// And, for the settings' "Re-test" as Thunderbird has it, a server tried
// without signing in: how it is reached, and how it lets one sign in.
use std::time::Duration;

use serde_json::{json, Value};

use super::imap::{Fail, Session};
use super::smtp::Smtp;
use super::{split, Auth, Security, Server};

// Mailboxes the lookup cannot find, by the address's domain: the IMAP host,
// port and way, and the SMTP ones. TU Dresden: its own Exchange (msx),
// neither in ISPDB nor with an autoconfig, its mail exchangers DFN's; the
// user name is the address (ZIH's FAQ), the password the ZIH one; it takes
// letters on 587 with STARTTLS (AUTH LOGIN once encrypted).
type Known = (&'static str, (&'static str, u16, Security), (&'static str, u16, Security));
const KNOWN: [Known; 2] = [
    ("tu-dresden.de", ("msx.tu-dresden.de", 993, Security::Ssl), ("msx.tu-dresden.de", 587, Security::StartTls)),
    ("mailbox.tu-dresden.de", ("msx.tu-dresden.de", 993, Security::Ssl), ("msx.tu-dresden.de", 587, Security::StartTls)),
];

fn known(domain: &str, address: &str) -> Option<(Server, Server)> {
    let (_, imap, smtp) = KNOWN.iter().find(|(d, ..)| *d == domain)?;
    let server = |(host, port, security): (&str, u16, Security)| Server { host: host.to_string(), port, security, username: address.into(), auth: Auth::Auto };
    Some((server(*imap), server(*smtp)))
}

// Found: the IMAP server, the SMTP one where there is one, and where they
// came from.
fn found(source: &str, server: &Server, smtp: Option<Server>) -> Value {
    json!({ "ok": true, "found": true, "source": source, "server": server.json(), "smtp": smtp.map(|s| s.json()) })
}

// What was found for an address: the servers, and where they came from
// (builtin, autoconfig, wellknown, ispdb, mx, guess); oauth when the provider
// takes only a browser sign-in for IMAP.
pub fn discover(address: &str) -> Value {
    let Some((_, domain)) = split(address) else { return json!({ "ok": false, "error": "address" }) };
    let address = address.trim();
    if let Some((server, smtp)) = known(&domain, address) {
        return found("builtin", &server, Some(smtp));
    }
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
                Some(Ok(server)) => return found(source, &server, outgoing(&xml, address)),
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
                Some(Ok(server)) => return found("mx", &server, outgoing(&xml, address)),
                Some(Err(())) => oauth = true,
                None => {}
            }
        }
    }
    if oauth {
        return json!({ "ok": true, "found": false, "oauth": true });
    }
    // A guess, where something answers as IMAP (and as SMTP).
    for host in [format!("imap.{domain}"), format!("mail.{domain}"), domain.clone()] {
        if let Some((security, port)) = probe(&host, false) {
            let server = Server { host, port, security, username: address.into(), auth: Auth::Auto };
            return found("guess", &server, guess_outgoing(&domain, address));
        }
    }
    json!({ "ok": true, "found": false })
}

// The outgoing server for an address, looked up as above: for an account
// set up before the pet sent mail.
pub fn outgoing_for(address: &str) -> Option<Server> {
    Server::from_json(&discover(address)["smtp"])
}

// The IMAP server in an autoconfig file: Some(Ok) one that takes a password,
// Some(Err) when they all take only OAuth, None when there is none.
fn from_config(xml: &str, address: &str) -> Option<Result<Server, ()>> {
    server_in(xml, "incomingServer", "imap", address)
}

// Its SMTP server that takes a password, if it has one.
fn outgoing(xml: &str, address: &str) -> Option<Server> {
    server_in(xml, "outgoingServer", "smtp", address)?.ok()
}

// The first server of a kind in an autoconfig file that takes a password
// and is encrypted, as from_config says.
fn server_in(xml: &str, tag: &str, kind: &str, address: &str) -> Option<Result<Server, ()>> {
    let (local, domain) = split(address)?;
    let fill = |s: &str| s.replace("%EMAILADDRESS%", address).replace("%EMAILLOCALPART%", local).replace("%EMAILDOMAIN%", &domain);
    let (open, close, of_kind) = (format!("<{tag}"), format!("</{tag}>"), format!("type=\"{kind}\""));
    let (ssl_port, starttls_port) = if kind == "smtp" { (465, 587) } else { (993, 143) };
    let mut oauth_only = false;
    let mut rest = xml;
    while let Some(at) = rest.find(&open) {
        let block = &rest[at..];
        let end = block.find(&close).unwrap_or(block.len());
        let (head, body) = block[..end].split_once('>').unwrap_or(("", ""));
        rest = &block[end..];
        if !head.contains(&of_kind) {
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
        let port = tags(body, "port").first().and_then(|p| p.parse().ok()).unwrap_or(if security == Security::Ssl { ssl_port } else { starttls_port });
        let username = tags(body, "username").first().map(|u| fill(u)).unwrap_or_else(|| address.into());
        return Some(Ok(Server { host: host.to_ascii_lowercase(), port, security, username, auth: Auth::Auto }));
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

// Something answering as IMAP at a host (over TLS on 993, else with
// STARTTLS on 143), or as SMTP (465, else 587).
fn probe(host: &str, smtp: bool) -> Option<(Security, u16)> {
    tries(None, None, smtp).into_iter().find(|&(security, port)| {
        let server = Server { host: host.into(), port, security, username: String::new(), auth: Auth::Auto };
        if smtp {
            Smtp::open(&server, Duration::from_secs(4)).map(Smtp::quit).is_ok()
        } else {
            Session::open(&server, Duration::from_secs(4)).map(Session::logout).is_ok()
        }
    })
}

// The outgoing server guessed, where something answers as SMTP.
fn guess_outgoing(domain: &str, address: &str) -> Option<Server> {
    [format!("smtp.{domain}"), format!("mail.{domain}"), domain.to_string()]
        .into_iter()
        .find_map(|host| probe(&host, true).map(|(security, port)| Server { host, port, security, username: address.into(), auth: Auth::Auto }))
}

// How a server lets one sign in, by what it can do, in Thunderbird's terms:
// plain (AUTHENTICATE PLAIN) and login (LOGIN, unless disabled) are its
// "Normal password", the only ones here; ntlm, gssapi (Kerberos), oauth2
// and cram (an encrypted password) it offers too, and are said so.
pub fn auths_of(caps: &[String]) -> Vec<&'static str> {
    let has = |c: &str| caps.iter().any(|x| x.eq_ignore_ascii_case(c));
    let mut out = Vec::new();
    if has("AUTH=PLAIN") {
        out.push("plain");
    }
    if !has("LOGINDISABLED") {
        out.push("login");
    }
    if has("AUTH=CRAM-MD5") {
        out.push("cram");
    }
    if has("AUTH=GSSAPI") {
        out.push("gssapi");
    }
    if has("AUTH=NTLM") {
        out.push("ntlm");
    }
    if has("AUTH=XOAUTH2") || has("AUTH=OAUTHBEARER") {
        out.push("oauth2");
    }
    out
}

// The same for an SMTP server, by AUTH's mechanisms as EHLO said them.
pub fn smtp_auths(mechanisms: &[String]) -> Vec<&'static str> {
    let has = |m: &str| mechanisms.iter().any(|x| x.eq_ignore_ascii_case(m));
    let names = [("PLAIN", "plain"), ("LOGIN", "login"), ("CRAM-MD5", "cram"), ("GSSAPI", "gssapi"), ("NTLM", "ntlm")];
    let mut out: Vec<&'static str> = names.iter().filter(|(m, _)| has(m)).map(|(_, n)| *n).collect();
    if has("XOAUTH2") || has("OAUTHBEARER") {
        out.push("oauth2");
    }
    out
}

// The ways to try a server, as Thunderbird's "Autodetect" would: the way
// and port given, else the usual port for the way, else (no way given) TLS
// before STARTTLS, never unencrypted. IMAP on 993 / 143, SMTP on 465 / 587
// (or 25, with STARTTLS).
fn tries(port: Option<u16>, security: Option<Security>, smtp: bool) -> Vec<(Security, u16)> {
    let (ssl, starttls) = if smtp { (465, 587) } else { (993, 143) };
    let usual = |s: Security| if s == Security::Ssl { ssl } else { starttls };
    match (security, port) {
        (Some(s), Some(p)) => vec![(s, p)],
        (Some(s), None) => vec![(s, usual(s))],
        (None, Some(p)) if p == ssl => vec![(Security::Ssl, p)],
        (None, Some(p)) if p == starttls || (smtp && p == 25) => vec![(Security::StartTls, p)],
        (None, Some(p)) => vec![(Security::Ssl, p), (Security::StartTls, p)],
        (None, None) => vec![(Security::Ssl, ssl), (Security::StartTls, starttls)],
    }
}

// A server tried without signing in: the first way that answers as IMAP
// (or SMTP), and how it lets one sign in; else why not.
pub fn probe_server(host: &str, port: Option<u16>, security: Option<Security>, smtp: bool) -> Value {
    let mut why = Fail::Connect("nothing to try".into());
    for (security, port) in tries(port, security, smtp) {
        let server = Server { host: host.into(), port, security, username: String::new(), auth: Auth::Auto };
        let opened = if smtp {
            Smtp::open(&server, Duration::from_secs(6)).map(|s| {
                let auths = smtp_auths(&s.auths());
                s.quit();
                auths
            })
        } else {
            Session::open(&server, Duration::from_secs(6)).map(|s| {
                let auths = auths_of(s.caps());
                s.logout();
                auths
            })
        };
        match opened {
            Ok(auths) => return json!({ "ok": true, "found": true, "security": security.name(), "port": port, "auths": auths }),
            Err(fail) => why = fail,
        }
    }
    json!({ "ok": true, "found": false, "error": why.json() })
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
        assert_eq!(server, Server { host: "imap.qq.com".into(), port: 993, security: Security::Ssl, username: "Me@qq.com".into(), auth: Auth::Auto });
        let local = QQ.replace("%EMAILADDRESS%", "%EMAILLOCALPART%");
        assert_eq!(from_config(&local, "me@qq.com").unwrap().unwrap().username, "me");
        let oauth = QQ.replace("password-cleartext", "OAuth2");
        assert_eq!(from_config(&oauth, "me@qq.com"), Some(Err(())));
        let both = QQ.replace("<authentication>password-cleartext</authentication>", "<authentication>OAuth2</authentication><authentication>password-cleartext</authentication>");
        assert!(matches!(from_config(&both, "me@qq.com"), Some(Ok(_))));
        assert_eq!(from_config("<clientConfig/>", "me@qq.com"), None);
    }

    #[test]
    fn a_config_gives_its_smtp_server_too() {
        let gmail = r#"<clientConfig><emailProvider id="googlemail.com">
          <incomingServer type="imap"><hostname>imap.gmail.com</hostname><port>993</port><socketType>SSL</socketType><username>%EMAILADDRESS%</username><authentication>OAuth2</authentication><authentication>password-cleartext</authentication></incomingServer>
          <outgoingServer type="smtp"><hostname>smtp.gmail.com</hostname><port>465</port><socketType>SSL</socketType><username>%EMAILADDRESS%</username><authentication>OAuth2</authentication><authentication>password-cleartext</authentication></outgoingServer>
        </emailProvider></clientConfig>"#;
        let smtp = outgoing(gmail, "me@gmail.com").unwrap();
        assert_eq!((smtp.host.as_str(), smtp.port, smtp.security, smtp.username.as_str()), ("smtp.gmail.com", 465, Security::Ssl, "me@gmail.com"));
        assert_eq!(from_config(gmail, "me@gmail.com").unwrap().unwrap().host, "imap.gmail.com");
        // Unencrypted, none; no port said, the usual one for the way.
        assert!(outgoing(&gmail.replace("<port>465</port><socketType>SSL</socketType>", "<port>25</port><socketType>plain</socketType>"), "me@gmail.com").is_none());
        assert_eq!(outgoing(&gmail.replace("<port>465</port><socketType>SSL", "<socketType>STARTTLS"), "me@gmail.com").unwrap().port, 587);
        assert!(outgoing(QQ, "me@qq.com").is_none());
    }

    #[test]
    fn some_mailboxes_are_known() {
        let (tud, out) = known("tu-dresden.de", "vorname.name@tu-dresden.de").unwrap();
        assert_eq!((tud.host.as_str(), tud.port, tud.security, tud.username.as_str()), ("msx.tu-dresden.de", 993, Security::Ssl, "vorname.name@tu-dresden.de"));
        assert_eq!((out.host.as_str(), out.port, out.security, out.username.as_str()), ("msx.tu-dresden.de", 587, Security::StartTls, "vorname.name@tu-dresden.de"));
        assert_eq!(known("mailbox.tu-dresden.de", "a.b@mailbox.tu-dresden.de").unwrap().0.host, "msx.tu-dresden.de");
        assert!(known("tu-dresden.de.evil.example", "x@tu-dresden.de.evil.example").is_none());
        let found = discover("Vorname.Name@TU-Dresden.de");
        assert_eq!((found["source"].as_str(), found["server"]["host"].as_str(), found["smtp"]["port"].as_u64()), (Some("builtin"), Some("msx.tu-dresden.de"), Some(587)));
        assert_eq!(outgoing_for("x@tu-dresden.de").map(|s| s.security), Some(Security::StartTls));
    }

    #[test]
    fn a_server_is_tried_as_autodetect_would() {
        assert_eq!(tries(None, None, false), [(Security::Ssl, 993), (Security::StartTls, 143)]);
        assert_eq!(tries(Some(993), None, false), [(Security::Ssl, 993)]);
        assert_eq!(tries(Some(143), None, false), [(Security::StartTls, 143)]);
        assert_eq!(tries(Some(1143), None, false), [(Security::Ssl, 1143), (Security::StartTls, 1143)]);
        assert_eq!(tries(None, Some(Security::Plain), false), [(Security::Plain, 143)]);
        assert_eq!(tries(Some(14310), Some(Security::Plain), false), [(Security::Plain, 14310)]);
        assert_eq!(tries(None, None, true), [(Security::Ssl, 465), (Security::StartTls, 587)]);
        assert_eq!(tries(Some(25), None, true), [(Security::StartTls, 25)]);
        assert_eq!(tries(None, Some(Security::StartTls), true), [(Security::StartTls, 587)]);
        let mechs = |s: &str| s.split(' ').map(String::from).collect::<Vec<_>>();
        assert_eq!(smtp_auths(&mechs("GSSAPI NTLM LOGIN")), ["login", "gssapi", "ntlm"]);
        assert_eq!(smtp_auths(&mechs("PLAIN LOGIN XOAUTH2")), ["plain", "login", "oauth2"]);
        // TU Dresden's Exchange, as it answers on 993 and on 143.
        let caps = |s: &str| s.split(' ').map(String::from).collect::<Vec<_>>();
        assert_eq!(auths_of(&caps("IMAP4 IMAP4rev1 AUTH=PLAIN AUTH=NTLM AUTH=GSSAPI SASL-IR IDLE")), ["plain", "login", "gssapi", "ntlm"]);
        assert_eq!(auths_of(&caps("IMAP4rev1 LOGINDISABLED STARTTLS")), Vec::<&str>::new());
        assert_eq!(auths_of(&caps("IMAP4rev1 AUTH=XOAUTH2 AUTH=OAUTHBEARER LOGINDISABLED")), ["oauth2"]);
    }

    #[test]
    fn mail_hosts_belong_to_their_domains() {
        assert_eq!(base_domain("mx1.qq.com"), "qq.com");
        assert_eq!(base_domain("a3011.mx.srv.dfn.de."), "dfn.de");
        assert_eq!(base_domain("mx.mail.sina.com.cn"), "sina.com.cn");
        assert_eq!(base_domain("mail.ox.ac.uk"), "ox.ac.uk");
    }
}
