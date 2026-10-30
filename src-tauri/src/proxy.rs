// Proxies: Windows' own, as Settings (Network, Proxy) has it and as Clash or
// v2rayN set it when their "system proxy" is on, read for the web (the
// updates, the crash reports, LHM's download, the plugins, the pets'
// gallery: web_ways, a URL for ureq, tried before going straight) and for
// mail (mail/proxy.rs: an address and a kind to go through); and a proxy as
// one is typed in. The registry's ProxyServer is "host:port" for every
// protocol, or one for each: "http=host:port;https=host:port;socks=host:port";
// ProxyOverride lists the hosts reached straight ("localhost;127.*;10.*;
// <local>", <local> being a name without a dot). A PAC script (AutoConfigURL,
// Clash's "PAC mode") is not read: as no proxy.
//
// Which way, the General page's Network proxy says (the setting netProxy,
// zhoupengjie/Wakuwaku#68): Windows' (the default), one typed in, or none;
// mail follows it unless its page or an account says otherwise.

use std::sync::Mutex;
use std::time::Duration;

use serde_json::{json, Value};
use ureq::config::ConfigBuilder;
use ureq::typestate::AgentScope;

// How a proxy is spoken to: HTTP's CONNECT, or SOCKS5.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Http,
    Socks5,
}

impl Kind {
    pub fn parse(s: &str) -> Option<Kind> {
        match s {
            "http" => Some(Kind::Http),
            "socks5" => Some(Kind::Socks5),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Kind::Http => "http",
            Kind::Socks5 => "socks5",
        }
    }
}

// A proxy: how it is spoken to, and where it is.
#[derive(Clone, Debug, PartialEq)]
pub struct Proxy {
    pub kind: Kind,
    pub host: String,
    pub port: u16,
}

impl Proxy {
    // As one writes it: 127.0.0.1:7890, [::1]:1080.
    pub fn at(&self) -> String {
        if self.host.contains(':') {
            format!("[{}]:{}", self.host, self.port)
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }

    // As typed: host:port (an IPv6 address in brackets), of the kind given
    // unless a scheme before it says (http://, socks5://, socks5h://,
    // socks://); a slash after is left off. None when it is not one.
    pub fn parse(text: &str, kind: Kind) -> Option<Proxy> {
        let text = text.trim();
        let (kind, rest) = match text.split_once("://") {
            Some((scheme, rest)) => {
                let kind = match scheme.to_ascii_lowercase().as_str() {
                    "http" => Kind::Http,
                    "socks" | "socks5" | "socks5h" => Kind::Socks5,
                    _ => return None,
                };
                (kind, rest)
            }
            None => (kind, text),
        };
        let rest = rest.trim_end_matches('/');
        let (host, port) = match rest.strip_prefix('[') {
            Some(v6) => v6.split_once("]:").filter(|(h, _)| h.contains(':'))?,
            None => rest.rsplit_once(':').filter(|(h, _)| !h.contains(':'))?,
        };
        let port = port.parse::<u16>().ok().filter(|p| *p > 0)?;
        let ok = !host.is_empty() && host.len() <= 253 && host.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ':'));
        ok.then(|| Proxy { kind, host: host.to_ascii_lowercase(), port })
    }
}

// A way to go, as set: { mode: system | manual | none, kind: http | socks5,
// address: "host:port" } (netProxy, mailProxy, an account's own).
#[derive(Clone, Debug, PartialEq, Default)]
pub enum Via {
    // Windows' proxy, read again at each connection: Clash's switched on or
    // off meanwhile is taken at the next.
    #[default]
    System,
    Manual(Proxy),
    Straight,
}

impl Via {
    // As kept in the settings; None when it is not one (a manual one with no
    // address that reads as one, a key of another name).
    pub fn of(v: &Value) -> Option<Via> {
        let m = v.as_object()?;
        if m.keys().any(|k| !matches!(k.as_str(), "mode" | "kind" | "address")) {
            return None;
        }
        match m.get("mode")?.as_str()? {
            "system" => Some(Via::System),
            "none" => Some(Via::Straight),
            "manual" => {
                let kind = Kind::parse(m.get("kind").and_then(Value::as_str).unwrap_or("http"))?;
                Proxy::parse(m.get("address")?.as_str()?, kind).map(Via::Manual)
            }
            _ => None,
        }
    }

    pub fn json(&self) -> Value {
        match self {
            Via::System => json!({ "mode": "system" }),
            Via::Straight => json!({ "mode": "none" }),
            Via::Manual(p) => json!({ "mode": "manual", "kind": p.kind.name(), "address": p.at() }),
        }
    }
}

// Windows' proxy as set: ProxyServer, ProxyOverride.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct System {
    pub server: String,
    pub bypass: String,
}

impl System {
    // One protocol's entry of ProxyServer ("https=…"), when it has them.
    fn entry(&self, scheme: &str) -> Option<&str> {
        self.server.split(';').find_map(|part| {
            let (name, value) = part.split_once('=')?;
            Some(value.trim()).filter(|v| name.trim().eq_ignore_ascii_case(scheme) && !v.is_empty())
        })
    }

    // For the web (the crash reports' relays), as a URL: the https entry,
    // else the http one, else the one for every protocol.
    pub fn for_web(&self) -> Option<String> {
        let pick = if self.server.contains('=') { self.entry("https").or_else(|| self.entry("http"))? } else { self.server.trim() };
        if pick.is_empty() {
            return None;
        }
        Some(if pick.contains("://") { pick.to_string() } else { format!("http://{pick}") })
    }

    // For mail, to any port: the socks entry where there is one (as Windows
    // means it, the proxy for other protocols), else the https or http one,
    // else the one for every protocol, which Windows speaks HTTP to (Clash
    // sets its mixed port, which takes either; v2rayN its HTTP port).
    pub fn for_mail(&self) -> Option<Proxy> {
        if !self.server.contains('=') {
            return Proxy::parse(&self.server, Kind::Http);
        }
        match self.entry("socks") {
            Some(socks) => Proxy::parse(socks, Kind::Socks5),
            None => Proxy::parse(self.entry("https").or_else(|| self.entry("http"))?, Kind::Http),
        }
    }

    // Whether a host is reached straight, by ProxyOverride: an entry as the
    // host is (* for any run of characters), or <local> for a name with no dot.
    pub fn bypasses(&self, host: &str) -> bool {
        self.bypass.split(';').map(str::trim).filter(|e| !e.is_empty()).any(|e| {
            if e.eq_ignore_ascii_case("<local>") {
                !host.contains('.') && !host.contains(':')
            } else {
                glob(e.as_bytes(), host.as_bytes())
            }
        })
    }
}

// A pattern with * against a name, letters in either case.
fn glob(pattern: &[u8], text: &[u8]) -> bool {
    match pattern.split_first() {
        None => text.is_empty(),
        Some((b'*', rest)) => (0..=text.len()).any(|i| glob(rest, &text[i..])),
        Some((c, rest)) => text.split_first().is_some_and(|(t, more)| t.eq_ignore_ascii_case(c) && glob(rest, more)),
    }
}

// Windows' proxy, when one is on (and is not only a PAC script).
#[cfg(windows)]
pub fn system() -> Option<System> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    let key = RegKey::predef(HKEY_CURRENT_USER).open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Internet Settings").ok()?;
    if key.get_value::<u32, _>("ProxyEnable").ok()? == 0 {
        return None;
    }
    let server: String = key.get_value("ProxyServer").ok()?;
    let bypass: String = key.get_value("ProxyOverride").unwrap_or_default();
    (!server.trim().is_empty()).then_some(System { server, bypass })
}

#[cfg(not(windows))]
pub fn system() -> Option<System> {
    None
}

// Windows' proxy as the web takes it, for the page: { kind, address }, or null.
pub fn web_system_view() -> Value {
    let p = system().and_then(|s| s.for_web()).and_then(|url| Proxy::parse(&url, Kind::Http));
    p.map_or(Value::Null, |p| json!({ "kind": p.kind.name(), "address": p.at() }))
}

// --- The web (updates, crash reports, LHM's download, the plugins, pets) --

// Every request of hers to the web goes one way, as netProxy says. Windows'
// (system): through Windows' proxy first where one is set (and none in the
// environment, which ureq takes by itself) and its list (ProxyOverride)
// does not name the host; then as ureq would; a host the list names
// straight. One typed in (manual): through it alone, our own way through
// (mail/proxy.rs tunnel: HTTP's CONNECT or SOCKS5, the name looked up by
// the proxy). None: straight. This computer itself always straight.
// PowerShell, which ran the plugins before, went Windows' way.

// netProxy as last set (set_web: as she starts, and when it is changed).
static WEB: Mutex<Via> = Mutex::new(Via::System);

pub fn set_web(v: &Value) {
    *WEB.lock().unwrap() = Via::of(v).unwrap_or_default();
}

pub fn web_setting() -> Via {
    WEB.lock().unwrap().clone()
}

// One way to try: through Windows' proxy (its URL), through the one typed
// in, as ureq would by itself (the environment's proxy if one is set there,
// else straight), or straight.
#[derive(Clone, Debug, PartialEq)]
pub enum Way {
    Through(String),
    Manual(Proxy),
    Default,
    Straight,
}

// Connecting at most so long, and the answer's head in so long after: a
// way that goes nowhere (one straight through a VPN's TUN may) fails in
// seconds, not at the end of a download's ten minutes.
const CONNECT: Duration = Duration::from_secs(15);
const HEAD: Duration = Duration::from_secs(30);

impl Way {
    // For the log: "through Windows' proxy http://127.0.0.1:7890",
    // "through the environment's proxy", "straight".
    pub fn name(&self) -> String {
        match self {
            Way::Through(p) => format!("through Windows' proxy {p}"),
            Way::Manual(p) => format!("through the proxy set, {} {}", p.kind.name(), p.at()),
            Way::Default if ureq::Proxy::try_from_env().is_some() => "through the environment's proxy".into(),
            Way::Default | Way::Straight => "straight".into(),
        }
    }

    // The proxy's address, for the page ("127.0.0.1:7890"); none straight.
    pub fn proxy_at(&self) -> Option<String> {
        match self {
            Way::Through(url) => Proxy::parse(url, Kind::Http).map(|p| p.at()),
            Way::Manual(p) => Some(p.at()),
            Way::Default | Way::Straight => None,
        }
    }

    // An agent for it, a whole call at most `timeout` (and CONNECT, HEAD),
    // its config as `tweak` makes it; none for a proxy ureq cannot take.
    pub fn agent_with(&self, timeout: Duration, tweak: impl FnOnce(ConfigBuilder<AgentScope>) -> ConfigBuilder<AgentScope>) -> Option<ureq::Agent> {
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(timeout))
            .timeout_connect(Some(CONNECT.min(timeout)))
            .timeout_recv_response(Some(HEAD.min(timeout)));
        Some(match self {
            Way::Through(p) => tweak(config.proxy(Some(ureq::Proxy::new(p).ok()?))).build().new_agent(),
            Way::Manual(p) => crate::mail::proxy::through(p, CONNECT.min(timeout), tweak(config)),
            Way::Straight => tweak(config.proxy(None)).build().new_agent(),
            Way::Default => tweak(config).build().new_agent(),
        })
    }

    // An agent for it: redirects followed as ureq does (10), statuses from
    // 400 errors.
    pub fn agent(&self, timeout: Duration) -> Option<ureq::Agent> {
        self.agent_with(timeout, |c| c)
    }

    // An agent for a plugin's waku.fetch: as agent_with, but a direct
    // connection resolves the name through Guard, which drops any address of
    // this computer or the network at home and connects to exactly what is
    // left — the same lookup ureq uses, so a name that answered one address
    // to the pre-check cannot answer an inside one to the connection
    // (addons.rs resolves_inside). Through a proxy the proxy resolves, so the
    // resolver is not consulted there (the pre-check is all, its residual
    // risk in docs/PLUGINS.md).
    pub fn guarded_agent(&self, timeout: Duration, tweak: impl FnOnce(ConfigBuilder<AgentScope>) -> ConfigBuilder<AgentScope>) -> Option<ureq::Agent> {
        use ureq::unversioned::resolver::DefaultResolver;
        use ureq::unversioned::transport::DefaultConnector;
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(timeout))
            .timeout_connect(Some(CONNECT.min(timeout)))
            .timeout_recv_response(Some(HEAD.min(timeout)));
        let built = |cfg: ConfigBuilder<AgentScope>| ureq::Agent::with_parts(cfg.build(), DefaultConnector::new(), Guard(DefaultResolver::default()));
        Some(match self {
            Way::Through(p) => built(tweak(config.proxy(Some(ureq::Proxy::new(p).ok()?)))),
            Way::Straight => built(tweak(config.proxy(None))),
            Way::Default => built(tweak(config)),
            Way::Manual(p) => crate::mail::proxy::through(p, CONNECT.min(timeout), tweak(config)),
        })
    }
}

// A resolver that looks a name up once and keeps only the addresses that are
// not this computer's nor the network's at home (addons::is_inside): ureq
// then connects to one of exactly those, keeping the name for Host and SNI,
// so there is no second lookup to point somewhere else.
#[derive(Debug)]
struct Guard(ureq::unversioned::resolver::DefaultResolver);

impl ureq::unversioned::resolver::Resolver for Guard {
    fn resolve(&self, uri: &ureq::http::Uri, config: &ureq::config::Config, timeout: ureq::unversioned::transport::NextTimeout) -> Result<ureq::unversioned::resolver::ResolvedSocketAddrs, ureq::Error> {
        let all = self.0.resolve(uri, config, timeout)?;
        let mut out = self.empty();
        for addr in &all {
            if !crate::addons::is_inside(addr.ip()) {
                out.push(*addr);
            }
        }
        if out.is_empty() {
            return Err(ureq::Error::HostNotFound);
        }
        Ok(out)
    }
}

// The host of an address, lower case; IPv6 without its brackets.
pub fn host_of(url: &str) -> Option<String> {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let authority = rest.split(['/', '?', '#']).next()?;
    let authority = authority.rsplit_once('@').map_or(authority, |(_, a)| a);
    let host = match authority.strip_prefix('[') {
        Some(v6) => v6.split_once(']').map(|(h, _)| h)?,
        None => authority.rsplit_once(':').map_or(authority, |(h, _)| h),
    };
    (!host.is_empty()).then(|| host.to_ascii_lowercase())
}

// This computer itself: never through a proxy.
pub fn is_loopback(host: &str) -> bool {
    host == "localhost" || host.ends_with(".localhost") || host == "::1" || host.starts_with("127.")
}

fn ways(host: &str, via: &Via, system: Option<System>, env_proxy: bool) -> Vec<Way> {
    if is_loopback(host) {
        return vec![Way::Straight];
    }
    match (via, system) {
        (Via::Manual(p), _) => vec![Way::Manual(p.clone())],
        (Via::Straight, _) => vec![Way::Straight],
        (Via::System, Some(s)) if s.bypasses(host) => vec![Way::Straight],
        (Via::System, Some(s)) if !env_proxy => s.for_web().map(Way::Through).into_iter().chain([Way::Default]).collect(),
        (Via::System, _) => vec![Way::Default],
    }
}

// The ways to try for an address, in turn, as netProxy and the system have
// them now.
pub fn web_ways(url: &str) -> Vec<Way> {
    web_ways_by(url, &web_setting())
}

// The same, for a way given (the page's test of one).
pub fn web_ways_by(url: &str, via: &Via) -> Vec<Way> {
    let system = if *via == Via::System { system() } else { None };
    ways(&host_of(url).unwrap_or_default(), via, system, ureq::Proxy::try_from_env().is_some())
}

// --- A GET by hand (her updates, the plugin market) -----------------------
// Moved here from updates.rs as it was, for the market (market.rs) to go the
// same way: each way in turn, redirects followed by hand so a failure says
// at which host, a 404 apart from the rest. `plain`: a redirect may go to
// http:// too (a test's server); else https only.

// At most so many redirects (GitHub's exe: one, to release-assets.githubusercontent.com).
const HOPS: usize = 5;

// A GET answered: the answer, the way it came (Way's name) and the
// hosts it went by ("github.com → release-assets.githubusercontent.com").
pub struct Answer {
    pub res: ureq::http::Response<ureq::Body>,
    pub way: String,
    pub path: String,
}

// A GET not answered: not there (404, as the server says it), or what went
// wrong each way it was tried: "<way>, <hosts>: <error>", "; " between.
pub enum Miss {
    NotThere,
    Failed(String),
}

// A GET, each way in turn (web_ways: Windows' proxy first, where one is
// set; then as is) till one is answered.
pub fn get(url: &str, seconds: u64, accept: Option<&str>, plain: bool) -> Result<Answer, Miss> {
    let mut failed = Vec::new();
    for way in web_ways(url) {
        match get_way(&way, url, seconds, accept, plain) {
            Ok(answer) => return Ok(answer),
            Err(None) => return Err(Miss::NotThere),
            Err(Some(e)) => failed.push(format!("{}, {e}", way.name())),
        }
    }
    Err(Miss::Failed(failed.join("; ")))
}

// One way: its redirects followed by hand (to https only, unless `plain`),
// so what goes wrong says at which host. Err(None): not there (404).
pub fn get_way(way: &Way, url: &str, seconds: u64, accept: Option<&str>, plain: bool) -> Result<Answer, Option<String>> {
    let agent = way.agent_with(Duration::from_secs(seconds), |c| c.max_redirects(0)).ok_or_else(|| Some(format!("{}: not a proxy to go through", way.name())))?;
    let mut at = url.to_string();
    let mut hosts = vec![host_of(url).unwrap_or_default()];
    for _ in 0..=HOPS {
        let path = hosts.join(" → ");
        let mut req = agent.get(&at).header("user-agent", &format!("wakuwaku/{}", crate::updates::current()));
        if let Some(accept) = accept {
            req = req.header("accept", accept);
        }
        let res = match req.call() {
            Err(ureq::Error::StatusCode(404)) => return Err(None),
            Err(e) => return Err(Some(format!("{path}: {e}"))),
            Ok(res) => res,
        };
        if !res.status().is_redirection() {
            return Ok(Answer { res, way: way.name(), path });
        }
        let to = res.headers().get("location").and_then(|l| l.to_str().ok()).and_then(|l| redirected(&at, l, plain));
        at = to.ok_or_else(|| Some(format!("{path}: a {} to nowhere it may go", res.status().as_u16())))?;
        hosts.push(host_of(&at).unwrap_or_default());
    }
    Err(Some(format!("{}: more than {HOPS} redirects", hosts.join(" → "))))
}

// Where a redirect goes from `from`: its Location made whole (one that
// starts with / or // is on the same host or scheme); https only, unless
// `plain`.
pub fn redirected(from: &str, to: &str, plain: bool) -> Option<String> {
    let to = to.trim();
    let (scheme, rest) = from.split_once("://")?;
    let url = if to.contains("://") {
        to.to_string()
    } else if let Some(net) = to.strip_prefix("//") {
        format!("{scheme}://{net}")
    } else if to.starts_with('/') {
        format!("{scheme}://{}{to}", rest.split('/').next().unwrap_or(""))
    } else {
        return None;
    };
    (url.starts_with("https://") || (plain && url.starts_with("http://"))).then_some(url)
}

// Whether an address is under one of these, as they are written (the
// scheme and host in either case): her releases' pages, the market's files.
pub fn is_under(url: &str, places: &[&str]) -> bool {
    places.iter().any(|p| url.get(..p.len()).is_some_and(|head| head.eq_ignore_ascii_case(p)))
}

// The page's Test: GitHub asked the way given (each of its ways in turn, as
// an update goes), any answer counting. { ok, via: the proxy it went
// through or null, ms } or { ok: false, via, error }; and, should it fail,
// a proxy listening on this computer at a port such apps use (Clash 7890,
// Clash Verge 7897, v2rayN 10809 HTTP, 10808 SOCKS), for the page to
// suggest: `listening`, { address, kind } or null.
pub fn test(via: &Via) -> Value {
    const URL: &str = "https://github.com/";
    let mut failed = json!({ "ok": false, "via": null, "error": "no way" });
    for way in web_ways_by(URL, via) {
        let started = std::time::Instant::now();
        let Some(agent) = way.agent_with(Duration::from_secs(10), |c| c.http_status_as_error(false).max_redirects(0)) else { continue };
        match agent.get(URL).header("user-agent", &format!("wakuwaku/{}", crate::updates::current())).call() {
            Ok(_) => return json!({ "ok": true, "via": way.proxy_at(), "ms": started.elapsed().as_millis() as u64 }),
            Err(e) => failed = json!({ "ok": false, "via": way.proxy_at(), "error": e.to_string() }),
        }
    }
    failed["listening"] = listening().map_or(Value::Null, |(port, kind)| json!({ "address": format!("127.0.0.1:{port}"), "kind": kind.name() }));
    failed
}

fn listening() -> Option<(u16, Kind)> {
    [(7890, Kind::Http), (7897, Kind::Http), (10809, Kind::Http), (10808, Kind::Socks5)].into_iter().find(|(port, _)| {
        std::net::TcpStream::connect_timeout(&std::net::SocketAddr::from(([127, 0, 0, 1], *port)), Duration::from_millis(300)).is_ok()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sys(server: &str) -> System {
        System { server: server.into(), bypass: String::new() }
    }

    #[test]
    fn what_clash_and_v2rayn_write_is_read() {
        // Clash (Verge, for Windows): its mixed port, for everything.
        let clash = sys("127.0.0.1:7890");
        assert_eq!(clash.for_mail(), Some(Proxy { kind: Kind::Http, host: "127.0.0.1".into(), port: 7890 }));
        assert_eq!(clash.for_web().as_deref(), Some("http://127.0.0.1:7890"));
        // v2rayN: its HTTP port.
        assert_eq!(sys("127.0.0.1:10809").for_mail().map(|p| p.at()).as_deref(), Some("127.0.0.1:10809"));
        // One for each protocol: mail through the socks one, the web through the https one.
        let each = sys("http=127.0.0.1:10809;https=127.0.0.1:10809;socks=127.0.0.1:10808");
        assert_eq!(each.for_mail(), Some(Proxy { kind: Kind::Socks5, host: "127.0.0.1".into(), port: 10808 }));
        assert_eq!(each.for_web().as_deref(), Some("http://127.0.0.1:10809"));
        assert_eq!(sys("http=127.0.0.1:8080;https=127.0.0.1:8443").for_mail().map(|p| (p.kind, p.port)), Some((Kind::Http, 8443)));
        assert_eq!(sys("ftp=1.2.3.4:21").for_mail(), None);
        assert_eq!(sys("SOCKS=localhost:1080").for_mail().map(|p| (p.kind, p.host)), Some((Kind::Socks5, "localhost".into())));
        assert_eq!(sys(" ").for_mail(), None);
        assert_eq!(sys("socks5://127.0.0.1:7891").for_mail().map(|p| p.kind), Some(Kind::Socks5));
    }

    #[test]
    fn some_hosts_are_reached_straight() {
        // Clash's list, shortened.
        let s = System { server: "127.0.0.1:7890".into(), bypass: "localhost;127.*;192.168.*;10.*;172.16.*;*.local;<local>".into() };
        assert!(s.bypasses("127.0.0.1"));
        assert!(s.bypasses("LOCALHOST"));
        assert!(s.bypasses("192.168.1.20"));
        assert!(s.bypasses("mail.corp.local"));
        assert!(s.bypasses("intranet"));
        assert!(!s.bypasses("imap.gmail.com"));
        assert!(!s.bypasses("10mail.example.com"));
        assert!(!sys("127.0.0.1:7890").bypasses("127.0.0.1"));
    }

    #[test]
    fn a_proxy_is_typed_as_one_writes_it() {
        let p = |t: &str| Proxy::parse(t, Kind::Http);
        assert_eq!(p(" 127.0.0.1:7890 "), Some(Proxy { kind: Kind::Http, host: "127.0.0.1".into(), port: 7890 }));
        assert_eq!(p("socks5://127.0.0.1:10808/").map(|x| (x.kind, x.port)), Some((Kind::Socks5, 10808)));
        assert_eq!(p("http://Proxy.Example.com:3128").map(|x| x.host), Some("proxy.example.com".into()));
        assert_eq!(p("[::1]:1080").map(|x| x.at()), Some("[::1]:1080".into()));
        for bad in ["127.0.0.1", "127.0.0.1:0", "127.0.0.1:99999", ":7890", "a b:1", "::1:1080", "https://x:1", "x:1 --y"] {
            assert_eq!(p(bad), None, "{bad}");
        }
    }

    #[test]
    fn hosts_are_read_from_addresses() {
        assert_eq!(host_of("http://localhost:3000").as_deref(), Some("localhost"));
        assert_eq!(host_of("https://API.github.com/repos/a/b").as_deref(), Some("api.github.com"));
        assert_eq!(host_of("http://[::1]:5173/api").as_deref(), Some("::1"));
        assert_eq!(host_of("http://user:pw@127.0.0.1:8080/x?y").as_deref(), Some("127.0.0.1"));
        assert_eq!(host_of("https://Release-Assets.githubusercontent.com:443/a?b").as_deref(), Some("release-assets.githubusercontent.com"));
        assert_eq!(host_of("http://").as_deref(), None);
        assert!(is_loopback("127.0.0.1") && is_loopback("localhost") && is_loopback("::1") && is_loopback("app.localhost"));
        assert!(!is_loopback("example.com") && !is_loopback("10.0.0.1"));
    }

    // This computer straight; Windows' proxy then as ureq would; straight
    // for a host its list names; the environment's alone when one is set
    // there.
    #[test]
    fn the_web_through_windows_proxy_first() {
        let clash = || Some(System { server: "127.0.0.1:7890".into(), bypass: "localhost;127.*;192.168.*;<local>".into() });
        let sys_way = &Via::System;
        assert_eq!(ways("localhost", sys_way, clash(), false), [Way::Straight]);
        assert_eq!(ways("github.com", sys_way, clash(), false), [Way::Through("http://127.0.0.1:7890".into()), Way::Default]);
        assert_eq!(ways("192.168.1.5", sys_way, clash(), false), [Way::Straight]);
        assert_eq!(ways("devbox", sys_way, clash(), false), [Way::Straight]);
        assert_eq!(ways("api.github.com", sys_way, None, false), [Way::Default]);
        assert_eq!(ways("api.github.com", sys_way, clash(), true), [Way::Default]);
        assert_eq!(ways("::1", sys_way, None, true), [Way::Straight]);
        assert_eq!(ways("127.0.0.1", sys_way, Some(sys("127.0.0.1:7890")), false), [Way::Straight]);
    }

    // One typed in (netProxy manual): through it alone, whatever Windows or
    // the environment say; none: straight, the environment's too. This
    // computer straight still.
    #[test]
    fn the_web_as_the_general_page_says() {
        let socks = Proxy { kind: Kind::Socks5, host: "127.0.0.1".into(), port: 10808 };
        let manual = Via::Manual(socks.clone());
        assert_eq!(ways("github.com", &manual, Some(sys("127.0.0.1:7890")), true), [Way::Manual(socks.clone())]);
        assert_eq!(ways("localhost", &manual, None, false), [Way::Straight]);
        assert_eq!(ways("github.com", &Via::Straight, Some(sys("127.0.0.1:7890")), true), [Way::Straight]);
        assert_eq!(Way::Manual(socks).proxy_at().as_deref(), Some("127.0.0.1:10808"));
        assert_eq!(Way::Through("http://127.0.0.1:7890".into()).proxy_at().as_deref(), Some("127.0.0.1:7890"));
        assert_eq!(Via::of(&json!({ "mode": "manual", "kind": "socks5", "address": "127.0.0.1:10808" })).map(|v| v.json()), Some(json!({ "mode": "manual", "kind": "socks5", "address": "127.0.0.1:10808" })));
        assert_eq!(Via::of(&json!({ "mode": "follow" })), None);
    }

    // A plugin's fetch straight to the web (guarded_agent): a name or an
    // address of this computer is not connected to at all, whatever the
    // pre-check said (addons.rs resolves_inside), as the same agent
    // unguarded would.
    #[test]
    fn a_guarded_agent_never_connects_inside() {
        use std::io::{ErrorKind, Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        listener.set_nonblocking(true).unwrap();
        let timeout = Duration::from_secs(5);
        for url in [format!("http://127.0.0.1:{port}/"), format!("http://localhost:{port}/"), format!("http://[::ffff:127.0.0.1]:{port}/")] {
            let agent = Way::Straight.guarded_agent(timeout, |c| c.http_status_as_error(false)).unwrap();
            let got = agent.get(&url).call();
            assert!(matches!(got, Err(ureq::Error::HostNotFound)), "{url}: {got:?}");
            assert_eq!(listener.accept().map(|_| ()).map_err(|e| e.kind()), Err(ErrorKind::WouldBlock), "{url}: connected");
        }
        // Unguarded, the same address is reached: the listener hears it.
        let heard = std::thread::spawn(move || {
            let until = std::time::Instant::now() + Duration::from_secs(5);
            loop {
                match listener.accept() {
                    Ok((mut s, _)) => {
                        s.set_nonblocking(false).unwrap();
                        let _ = s.read(&mut [0; 1024]);
                        s.write_all(b"HTTP/1.1 204 No Content\r\nconnection: close\r\n\r\n").unwrap();
                        return true;
                    }
                    Err(_) if std::time::Instant::now() < until => std::thread::sleep(Duration::from_millis(10)),
                    Err(_) => return false,
                }
            }
        });
        let agent = Way::Straight.agent(timeout).unwrap();
        assert_eq!(agent.get(&format!("http://127.0.0.1:{port}/")).call().map(|r| r.status().as_u16()).ok(), Some(204));
        assert!(heard.join().unwrap());
    }
}
