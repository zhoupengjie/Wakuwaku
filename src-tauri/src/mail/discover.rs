// Finding an address's IMAP server the way Thunderbird does: the domain's
// own autoconfig, Thunderbird's database ISPDB, the database's entry for the
// provider of the domain's MX host, then a guess where something answers as
// IMAP.
use std::time::Duration;

use serde_json::{json, Value};

use super::imap::Session;
use super::{split, Security, Server};


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
        Session::open(&server, Duration::from_secs(4)).is_ok()
    };
    if try_one(Security::Ssl, 993) {
        return Some((Security::Ssl, 993));
    }
    try_one(Security::StartTls, 143).then_some((Security::StartTls, 143))
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
}
