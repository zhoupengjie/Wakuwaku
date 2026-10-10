// Mail in the island, built in. Accounts are set up in the settings the way
// Thunderbird does it: an address and a password, then the server looked up
// (the domain's own autoconfig, Thunderbird's database ISPDB, the database's
// entry for the provider of the domain's MX host, then a guess) or typed in.
// Each account that is on is watched over IMAP by a thread of its own: the
// unread count and the newest unread letter (who, what about) as a widget,
// and the island opened for a new one, at once where the server pushes
// (IDLE), else every minute. The watch reads only: the inbox is EXAMINEd and
// headers are read with BODY.PEEK, so nothing is marked read. Passwords are
// kept in Windows' Credential Manager ("Wakuwaku mail <id>"), never in the
// settings.
//
// The Mail page also lists an inbox and reads a letter (letters.rs; opening
// one marks it read, as any mail program does), and hands a letter to Claude
// Code or Codex (agent.rs). IMAP is io-imap's (imap.rs), the library under
// himalaya; finding the server is discover.rs.
//
// Settings: "mail": [{ id, address, host, port, security: ssl | starttls |
// plain, username, on }], changed only through the commands below;
// "mailAgent": "claude" | "codex", who a letter goes to first;
// "mailAgentConf": { claude | codex: { access, model, effort, sumModel,
// sumEffort } }, what a session may do and the models (agent.rs).
use std::collections::HashMap;
use std::net::{Shutdown, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tauri::AppHandle;

use crate::{i18n, now_ms, settings, shared, Shared};

pub mod agent;
mod discover;
mod imap;
pub mod letters;

pub use discover::discover;
use imap::{session, Fail};

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

// How to sign in, as Thunderbird's "Authentication method" has it: auto
// (AUTHENTICATE PLAIN where offered, else LOGIN), or one of them always.
// Both are Thunderbird's "Normal password"; the others it offers (NTLM,
// Kerberos, OAuth2, an encrypted password) are not here.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum Auth {
    #[default]
    Auto,
    Plain,
    Login,
}

impl Auth {
    fn parse(s: &str) -> Auth {
        match s {
            "plain" => Auth::Plain,
            "login" => Auth::Login,
            _ => Auth::Auto,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Auth::Auto => "auto",
            Auth::Plain => "plain",
            Auth::Login => "login",
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Server {
    pub host: String,
    pub port: u16,
    pub security: Security,
    pub username: String,
    pub auth: Auth,
}

// A host name as one is written: letters, digits, dots, dashes.
fn host_of(v: &Value) -> Option<String> {
    let host = v.as_str()?.trim().to_ascii_lowercase();
    (!host.is_empty() && host.len() <= 253 && host.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')).then_some(host)
}

fn port_of(v: &Value) -> Option<u16> {
    v.as_u64().or_else(|| v.as_str()?.trim().parse().ok()).filter(|p| (1..=65535).contains(p)).map(|p| p as u16)
}

impl Server {
    fn json(&self) -> Value {
        json!({ "host": self.host, "port": self.port, "security": self.security.name(), "username": self.username, "auth": self.auth.name() })
    }

    // As the page sent it, checked: a host name, a port, a way, a user, how to sign in.
    fn from_json(v: &Value) -> Option<Server> {
        let host = host_of(&v["host"])?;
        let port = port_of(&v["port"])?;
        let username = v["username"].as_str()?.trim();
        let is_user = !username.is_empty() && username.chars().count() <= 320 && !username.chars().any(char::is_control);
        let security = Security::parse(v["security"].as_str().unwrap_or("")).unwrap_or(Security::Ssl);
        is_user.then(|| Server { host, port, security, username: username.into(), auth: Auth::parse(v["auth"].as_str().unwrap_or("")) })
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

// A header block's sender (the name, else the address) and subject, its
// encoded words and charsets read.
fn from_and_subject(head: &[u8]) -> (String, String) {
    let Some(message) = mail_parser::MessageParser::default().parse(head) else { return (String::new(), String::new()) };
    let from = message.from().and_then(|a| a.first()).and_then(|a| a.name().or(a.address())).unwrap_or("").trim().to_string();
    (from, message.subject().unwrap_or("").trim().to_string())
}

// A try before an account is kept: signed in, the inbox opened read-only,
// and the unread counted; or what went wrong, with how the server lets one
// sign in (the page says more when that is a Windows domain's).
fn test(server: &Server, password: &str) -> Result<usize, (Fail, Vec<&'static str>)> {
    let mut s = imap::Session::open(server, CONNECT).map_err(|f| (f, Vec::new()))?;
    let auths = discover::auths_of(s.caps());
    let tried = (|| {
        s.login(&server.username, password, server.auth)?;
        s.inbox(false)?;
        s.unseen()
    })();
    match tried {
        Ok(unseen) => {
            s.logout();
            Ok(unseen.len())
        }
        Err(fail) => Err((fail, auths)),
    }
}

// An account's password, as kept.
fn password_of(account: &Account) -> Result<String, Fail> {
    secret::read(&account.id).ok_or_else(|| Fail::Login("no password kept".into()))
}

// The account with this id.
fn account(sh: &Shared, id: &str) -> Option<Account> {
    accounts(sh).into_iter().find(|a| a.id == id)
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
    // Talks with an agent about a letter (agent.rs).
    talks: agent::Talks,
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
            let mut conn = session(&account.server, &password_of(account)?, CONNECT)?;
            *watch.socket.lock().unwrap() = conn.socket.try_clone().ok();
            if watch.stopped() {
                return Ok(());
            }
            conn.inbox(false)?;
            failures = 0;
            loop {
                let unseen = conn.unseen()?;
                let newest = unseen.iter().max().copied();
                let letter = match newest {
                    Some(uid) => conn.heads(&uid.to_string(), true, &["FROM", "SUBJECT"])?.first().map(|h| from_and_subject(&h.header)),
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
                    conn.idle(IDLE_FOR)?;
                } else {
                    watch.rest(POLL);
                    if watch.stopped() {
                        return Ok(());
                    }
                    conn.noop()?;
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

// The accounts are read, changed and written back by one command at a time:
// two saves at once (the sign-in between taking seconds) would each write
// back the list as it was before the other.
static EDITING: Mutex<()> = Mutex::new(());

// An id no account has: the time, a count and the process, mixed.
fn new_id(all: &[Account]) -> String {
    use std::hash::{BuildHasher, Hasher};
    static COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    loop {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u64(now_ms());
        h.write_u64(COUNT.fetch_add(1, Ordering::SeqCst));
        h.write_u32(std::process::id());
        let id = format!("{:08x}", h.finish() & 0xffff_ffff);
        if !all.iter().any(|a| a.id == id) {
            return id;
        }
    }
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

// "Re-test", as Thunderbird has it: the server tried without signing in,
// the port and the way as given or (empty, "auto") found.
#[tauri::command]
pub async fn mail_probe(host: Value, port: Value, security: String) -> Value {
    let Some(host) = host_of(&host) else { return json!({ "ok": false, "error": { "kind": "fields", "text": "" } }) };
    let port = port_of(&port);
    let security = Security::parse(&security);
    tauri::async_runtime::spawn_blocking(move || discover::probe_server(&host, port, security)).await.unwrap_or_else(|e| json!({ "ok": false, "error": { "kind": "other", "text": e.to_string() } }))
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
    // One being changed: its id, and the password kept for it.
    let known = account["id"].as_str().filter(|id| accounts(&sh).iter().any(|a| a.id == *id)).map(String::from);
    let password = if password.is_empty() { known.as_deref().and_then(secret::read).unwrap_or_default() } else { password };
    if password.is_empty() {
        return json!({ "ok": false, "error": { "kind": "password", "text": "" } });
    }
    let (s, p) = (server.clone(), password.clone());
    let tried = tauri::async_runtime::spawn_blocking(move || test(&s, &p)).await.unwrap_or_else(|e| Err((Fail::Other(e.to_string()), Vec::new())));
    let unread = match tried {
        Ok(n) => n,
        Err((fail, auths)) => return json!({ "ok": false, "error": fail.json(), "auths": auths }),
    };
    // Kept against the accounts as they are now, others saved meanwhile included.
    {
        let _editing = EDITING.lock().unwrap();
        let mut all = accounts(&sh);
        let id = known.filter(|id| all.iter().any(|a| a.id == *id)).unwrap_or_else(|| new_id(&all));
        if let Err(e) = secret::write(&id, &password) {
            return json!({ "ok": false, "error": { "kind": "keep", "text": e } });
        }
        let kept = Account { id: id.clone(), address, server, on: true };
        match all.iter_mut().find(|a| a.id == id) {
            Some(a) => *a = kept,
            None => all.push(kept),
        }
        keep_accounts(&sh, &all);
    }
    json!({ "ok": true, "unread": unread, "snapshot": settings::snapshot(&sh) })
}

#[tauri::command]
pub async fn mail_remove(app: AppHandle, id: String) -> Value {
    let sh = shared(&app);
    {
        let _editing = EDITING.lock().unwrap();
        let all: Vec<Account> = accounts(&sh).into_iter().filter(|a| a.id != id).collect();
        secret::delete(&id);
        keep_accounts(&sh, &all);
    }
    settings::snapshot(&sh)
}

#[tauri::command]
pub async fn mail_switch(app: AppHandle, id: String, on: bool) -> Value {
    let sh = shared(&app);
    {
        let _editing = EDITING.lock().unwrap();
        let mut all = accounts(&sh);
        if let Some(a) = all.iter_mut().find(|a| a.id == id) {
            a.on = on;
        }
        keep_accounts(&sh, &all);
    }
    settings::snapshot(&sh)
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

    #[test]
    fn a_letter_says_who_and_what_about() {
        let head = "From: =?UTF-8?B?546L5oC7?= <boss@example.com>\r\nSubject: =?GBK?B?1tzO5bXEt72wuA==?=\r\n\r\n";
        assert_eq!(from_and_subject(head.as_bytes()), ("王总".into(), "周五的方案".into()));
        assert_eq!(from_and_subject(b"From: boss@example.com\r\n\r\n").0, "boss@example.com");
    }

    #[test]
    fn two_accounts_at_once_get_ids_of_their_own() {
        let server = Server { host: "h".into(), port: 993, security: Security::Ssl, username: "u".into(), auth: Auth::Auto };
        let mut all = Vec::new();
        for _ in 0..2000 {
            let id = new_id(&all);
            assert!(id.len() == 8 && !all.iter().any(|a: &Account| a.id == id), "{id}");
            all.push(Account { id, address: "a@b.c".into(), server: server.clone(), on: true });
        }
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
