// Third-party plugins (addons/mod.rs). Hosts: which a plugin may name and reach.
use super::*;

// --- Hosts: which a plugin may name and reach ------------------------------------------

// This computer's, or a name of the network's at home: one with no dot, or
// under one of HOME_NAMES.
pub(super) fn is_home_name(host: &str) -> bool {
    !host.contains('.') || HOME_NAMES.iter().any(|e| host == *e || host.strip_suffix(e).is_some_and(|rest| rest.ends_with('.')))
}

// A name on the internet, lower case: labels of a-z 0-9 -, at least two, the
// last with a letter (so no address, 127.0.0.1 nor 2130706433), not at home.
pub(super) fn is_public_name(host: &str) -> bool {
    let labels: Vec<&str> = host.split('.').collect();
    host.len() <= 253
        && labels.len() >= 2
        && labels.iter().all(|l| (1..=63).contains(&l.len()) && !l.starts_with('-') && !l.ends_with('-') && l.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'))
        && labels.last().is_some_and(|tld| tld.bytes().any(|b| b.is_ascii_lowercase()))
        && !is_home_name(host)
}

// A host its manifest may name: a name, or *.<name>; no port, no address.
pub fn is_host_rule(rule: &str) -> bool {
    is_public_name(rule.strip_prefix("*.").unwrap_or(rule))
}

// Whether the hosts named let it reach this one: the same name, or one
// under *.<name> (not <name> itself).
pub fn host_allowed(rules: &[String], host: &str) -> bool {
    rules.iter().any(|r| match r.strip_prefix("*.") {
        Some(base) => host.strip_suffix(base).is_some_and(|rest| rest.len() > 1 && rest.ends_with('.')),
        None => host == r,
    })
}

// An address of this computer or of the network at home: loopback, private,
// link-local, carrier-grade NAT, unspecified, multicast, broadcast; IPv6's
// unique-local and link-local, and an IPv4 one reachable through IPv6 (a
// mapped or compatible ::a.b.c.d, a 6to4 2002::/16 or a NAT64 64:ff9b::/96
// address carrying one of those IPv4 addresses).
pub(crate) fn is_inside(ip: std::net::IpAddr) -> bool {
    use std::net::{IpAddr, Ipv4Addr};
    match ip {
        IpAddr::V4(v4) => {
            let [a, b, ..] = v4.octets();
            v4.is_loopback() || v4.is_private() || v4.is_link_local() || v4.is_unspecified() || v4.is_broadcast() || v4.is_multicast() || a == 0 || (a == 100 && (64..128).contains(&b))
        }
        IpAddr::V6(v6) => {
            let seg = v6.segments();
            let first = seg[0];
            // An IPv4 address carried in this IPv6 one, if any: a mapped or
            // compatible ::a.b.c.d (to_ipv4), a 6to4 2002:AABB:CCDD::, or the
            // well-known NAT64 prefix 64:ff9b::/96.
            let in_v4 = |a, b, c, d| Ipv4Addr::new(a, b, c, d);
            let carried = if first == 0x2002 {
                Some(in_v4((seg[1] >> 8) as u8, seg[1] as u8, (seg[2] >> 8) as u8, seg[2] as u8))
            } else if first == 0x0064 && seg[1] == 0xff9b && seg[2..6].iter().all(|&s| s == 0) {
                Some(in_v4((seg[6] >> 8) as u8, seg[6] as u8, (seg[7] >> 8) as u8, seg[7] as u8))
            } else {
                v6.to_ipv4()
            };
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || (first & 0xfe00) == 0xfc00
                || (first & 0xffc0) == 0xfe80
                || carried.is_some_and(|v4| is_inside(IpAddr::V4(v4)))
        }
    }
}

// Where a request goes: http or https, a name, port 80 or 443, its path.
#[derive(Debug, PartialEq)]
pub(super) struct Target {
    pub(super) url: String,
    pub(super) host: String,
    pub(super) port: u16,
}

// An address a plugin gives, checked: http(s), a name (no address, no user
// or password), port 80 or 443; its fragment left off.
pub(super) fn target_of(url: &str) -> Result<Target, String> {
    let url = url.trim();
    let (scheme, rest) = url.split_once("://").ok_or("not an http(s) address")?;
    let scheme = scheme.to_ascii_lowercase();
    if scheme != "http" && scheme != "https" {
        return Err("only http and https".into());
    }
    let rest = rest.split('#').next().unwrap_or("");
    let end = rest.find(['/', '?']).unwrap_or(rest.len());
    let (authority, path) = rest.split_at(end);
    if authority.contains('@') {
        return Err("no user or password in the address".into());
    }
    if authority.starts_with('[') {
        return Err("an address, not a name".into());
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) => (h, p.parse::<u16>().map_err(|_| "a port that is none")?),
        None => (authority, if scheme == "https" { 443 } else { 80 }),
    };
    if port != 80 && port != 443 {
        return Err(format!("port {port}: only 80 and 443"));
    }
    let host = host.to_ascii_lowercase();
    if host.parse::<std::net::Ipv4Addr>().is_ok() || !is_public_name(&host) {
        return Err(format!("{host}: not a name on the internet"));
    }
    if path.chars().any(|c| c.is_whitespace() || c.is_control() || c == '\\') {
        return Err("spaces in the address: encode them".into());
    }
    let shown_port = if (scheme == "https" && port == 443) || (scheme == "http" && port == 80) { String::new() } else { format!(":{port}") };
    let path = if path.is_empty() { "/" } else { path };
    let path = if path.starts_with('?') { format!("/{path}") } else { path.to_string() };
    Ok(Target { url: format!("{scheme}://{host}{shown_port}{path}"), host, port })
}

// Where a redirect goes from `from`: its Location made whole.
pub(super) fn redirected(from: &str, to: &str) -> Option<String> {
    let to = to.trim();
    let (scheme, rest) = from.split_once("://")?;
    let origin = format!("{scheme}://{}", rest.split(['/', '?']).next().unwrap_or(""));
    Some(if to.contains("://") {
        to.to_string()
    } else if let Some(net) = to.strip_prefix("//") {
        format!("{scheme}://{net}")
    } else if to.starts_with('/') {
        format!("{origin}{to}")
    } else {
        let path = rest.split('?').next().unwrap_or("");
        let dir = path.rfind('/').map_or(String::new(), |i| path[..=i].to_string());
        let dir = dir.split_once('/').map_or("/".to_string(), |(_, d)| format!("/{d}"));
        format!("{origin}{dir}{to}")
    })
}

// One way to ask the web, for a plugin (addon_fetch).
pub struct Ask {
    pub url: String,
    pub method: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
}

// The request a plugin asked for, made as its permissions let it: each hop
// (the address, then each redirect) a host its manifest names, a name on
// the internet at port 80 or 443, none of whose addresses is this
// computer's or the network's at home; the way her other requests go
// (proxy::web_ways), each tried in turn. { status, ok, headers, text }, or
// what would not do.
pub fn fetch(perms: &Perms, ask: Ask) -> Result<Value, String> {
    let method = ask.method.trim().to_ascii_uppercase();
    let method = if method.is_empty() { "GET".to_string() } else { method };
    if !matches!(method.as_str(), "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD") {
        return Err(format!("{method}: GET, POST, PUT, PATCH, DELETE or HEAD"));
    }
    let mut body = ask.body.filter(|b| !b.is_empty());
    if body.as_ref().is_some_and(|b| b.len() > FETCH_MAX) {
        return Err("a body over 1 MB".into());
    }
    if body.is_some() && matches!(method.as_str(), "GET" | "HEAD") {
        return Err(format!("a {method} has no body"));
    }
    let mut headers = Vec::new();
    for (k, v) in ask.headers.into_iter().take(32) {
        let name = k.trim().to_ascii_lowercase();
        let ok = !name.is_empty() && name.len() <= 64 && name.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_!#$%&'*+.^`|~".contains(&b));
        if !ok || v.len() > 4096 || v.chars().any(|c| c == '\r' || c == '\n' || c == '\0') {
            return Err(format!("{k}: not a header"));
        }
        if !HEADERS_NOT.contains(&name.as_str()) {
            headers.push((name, v));
        }
    }
    if !headers.iter().any(|(k, _)| k == "user-agent") {
        headers.push(("user-agent".into(), format!("wakuwaku/{}", env!("CARGO_PKG_VERSION"))));
    }
    let until = std::time::Instant::now() + FETCH_TIME;
    let mut method = method;
    let mut url = ask.url;
    for _ in 0..=HOPS {
        let target = target_of(&url)?;
        if !host_allowed(&perms.net, &target.host) {
            return Err(format!("{}: not a host its manifest names (permissions.net)", target.host));
        }
        if resolves_inside(&target.host, target.port) {
            return Err(format!("{}: an address of this computer or the network at home", target.host));
        }
        let res = send(&target.url, &method, &headers, body.as_deref(), until)?;
        let status = res.status().as_u16();
        if res.status().is_redirection() && status != 304 {
            let to = res.headers().get("location").and_then(|l| l.to_str().ok()).and_then(|l| redirected(&target.url, l)).ok_or(format!("a {status} to nowhere"))?;
            // As a browser does: 303 (and 301, 302 after a POST) as a GET, without its body.
            if status == 303 || (matches!(status, 301 | 302) && method == "POST") {
                method = "GET".into();
                body = None;
            }
            url = to;
            continue;
        }
        return answer(res);
    }
    Err(format!("more than {HOPS} redirects"))
}

// Whether a name is one of this computer or of the network at home, as the
// system looks it up now (a name of the internet made to point there). Not
// found here, it is left to the way it goes (a proxy may know it).
pub(super) fn resolves_inside(host: &str, port: u16) -> bool {
    use std::net::ToSocketAddrs;
    (host, port).to_socket_addrs().is_ok_and(|mut addrs| addrs.any(|a| is_inside(a.ip())))
}

pub(super) fn send(url: &str, method: &str, headers: &[(String, String)], body: Option<&str>, until: std::time::Instant) -> Result<ureq::http::Response<ureq::Body>, String> {
    // A request that is not idempotent (a POST and the like) may have reached
    // the server and been acted on even when its answer never came, so it is
    // tried another way only when the first never opened its connection
    // (a proxy down, the name not found): else one plugin POST could be sent
    // twice. A GET or HEAD may always be tried again.
    let idempotent = matches!(method, "GET" | "HEAD");
    let mut last = "no way to the web".to_string();
    for way in crate::proxy::web_ways(url) {
        let left = until.saturating_duration_since(std::time::Instant::now());
        if left.is_zero() {
            return Err("timed out".into());
        }
        let Some(agent) = way.guarded_agent(left, |c| c.http_status_as_error(false).max_redirects(0).max_redirects_will_error(false)) else { continue };
        let mut req = ureq::http::Request::builder().method(method).uri(url);
        for (k, v) in headers {
            req = req.header(k.as_str(), v.as_str());
        }
        let sent = match body {
            Some(b) => req.body(b.to_string()).map_err(|e| e.to_string()).map(|r| agent.run(r)),
            None => req.body(()).map_err(|e| e.to_string()).map(|r| agent.run(r)),
        };
        match sent {
            Err(e) => return Err(e), // the request could not be built
            Ok(Ok(res)) => return Ok(res),
            // The connection had opened and this is not idempotent: the
            // server may have acted on it, so it is not tried another way.
            Ok(Err(e)) if !idempotent && !before_server(&e) => return Err(words(&e)),
            Ok(Err(e)) => last = words(&e),
        }
    }
    Err(last)
}

// Whether a failure came before the request could reach the server: the
// connection was never opened (the name not found, the connection refused
// or timed out while opening, no proxy to go through). Such a request is
// safe to try another way even when it is not idempotent.
pub(super) fn before_server(e: &ureq::Error) -> bool {
    match e {
        ureq::Error::Timeout(t) => matches!(t, ureq::Timeout::Connect | ureq::Timeout::Resolve),
        ureq::Error::HostNotFound | ureq::Error::ConnectionFailed => true,
        ureq::Error::Io(io) => matches!(io.kind(), std::io::ErrorKind::ConnectionRefused | std::io::ErrorKind::AddrNotAvailable | std::io::ErrorKind::NetworkUnreachable | std::io::ErrorKind::HostUnreachable),
        _ => false,
    }
}

pub(super) fn words(e: &ureq::Error) -> String {
    match e {
        ureq::Error::Io(io) => io.to_string(),
        ureq::Error::Timeout(_) => "timed out".into(),
        ureq::Error::HostNotFound => "host not found".into(),
        other => other.to_string(),
    }
}

pub(super) fn answer(mut res: ureq::http::Response<ureq::Body>) -> Result<Value, String> {
    let status = res.status().as_u16();
    let mut headers = Map::new();
    for name in HEADERS_BACK {
        if let Some(v) = res.headers().get(name).and_then(|v| v.to_str().ok()) {
            headers.insert(name.into(), json!(v));
        }
    }
    let bytes = res.body_mut().with_config().limit(FETCH_MAX as u64).read_to_vec().map_err(|e| match e {
        ureq::Error::BodyExceedsLimit(_) => "an answer over 1 MB".to_string(),
        e => words(&e),
    })?;
    Ok(json!({ "status": status, "ok": (200..300).contains(&status), "headers": headers, "text": String::from_utf8_lossy(&bytes) }))
}

