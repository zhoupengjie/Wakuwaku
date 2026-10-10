// Keeping an account's store as its server has it (store.rs), the way
// Thunderbird keeps an IMAP account offline: a thread for each account
// that is on, over a connection of its own.
// - Each folder, the inbox then the Sent folder, opened read-only (EXAMINE):
//   another UIDVALIDITY, and it is begun again. Then every letter's UID and
//   flags (UID FETCH 1:* FLAGS): the flags taken (not over ones just changed
//   here), letters no longer there gone, letters not kept yet asked for by
//   their header fields, newest first, a hundred at a time (the page shows
//   them as they come).
// - Then every letter not yet kept whole comes down (BODY.PEEK[]), newest
//   first, twenty seconds at a time, with a look for new mail in between
//   (and every two minutes the inbox's flags again).
// - Then it waits on the inbox: IDLE where the server pushes, else a look
//   every minute; when the server says anything, all again. The Sent folder
//   every ten minutes, or at once when a letter was sent (kick).
// Nothing is marked read here: folders are EXAMINEd and letters PEEKed.
// The account's widget (unread, the newest unread letter, new mail opening
// the island) comes from the store, at once from what was kept.
use std::collections::HashSet;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use io_imap::types::mailbox::Mailbox;
use serde_json::json;

use super::imap::{self, session, Fail, Session};
use super::store::{self, Entry, Folder, Store};
use super::{password_of, show, Account, Watch, CONNECT, IDLE_FOR, POLL};
use crate::Shared;

// The header fields a letter is kept by.
pub const FIELDS: [&str; 9] = ["FROM", "TO", "CC", "SUBJECT", "DATE", "MESSAGE-ID", "IN-REPLY-TO", "REFERENCES", "CONTENT-TYPE"];
const HEADS_AT_ONCE: usize = 100;
const BODIES_FOR: Duration = Duration::from_secs(20);
const SENT_EVERY: Duration = Duration::from_secs(600);
// While a big mailbox comes down, its flags taken again this often (read,
// starred, deleted elsewhere meanwhile).
const FLAGS_EVERY: Duration = Duration::from_secs(120);

pub fn run(sh: &Arc<Shared>, account: &Account, watch: &Watch) {
    let st = store::of(sh, account);
    // The newest unread letter seen: one newer is new mail.
    let mut newest_seen: Option<u32> = None;
    tell(sh, account, watch, &st, "connecting", &mut None);
    let mut failures = 0u32;
    while !watch.stopped() {
        // Letters that would not come down (the server said no): not asked again this time.
        let mut skip: HashSet<(Folder, u32)> = HashSet::new();
        let result = (|| -> Result<(), Fail> {
            let mut conn = session(&account.server, &password_of(account)?, CONNECT)?;
            *watch.socket.lock().unwrap() = conn.socket.try_clone().ok();
            failures = 0;
            let mut sent_at: Option<Instant> = None;
            loop {
                if watch.stopped() {
                    return Ok(());
                }
                sync_folder(sh, &mut conn, &st, Folder::Inbox, Mailbox::Inbox)?;
                let kicked = watch.kick.swap(false, Ordering::SeqCst);
                if kicked || sent_at.is_none_or(|t| t.elapsed() >= SENT_EVERY) {
                    sync_sent(sh, &mut conn, &st)?;
                    sent_at = Some(Instant::now());
                }
                save(&st);
                tell(sh, account, watch, &st, "ok", &mut newest_seen);
                // Whole letters, a while at a time; new mail between, and
                // now and then the inbox's flags.
                let mut flags_at = Instant::now();
                while !watch.stopped() && !watch.kick.load(Ordering::SeqCst) {
                    let left = download(&mut conn, &st, watch, &mut skip)?;
                    save(&st);
                    let came = if flags_at.elapsed() >= FLAGS_EVERY {
                        flags_at = Instant::now();
                        sync_folder(sh, &mut conn, &st, Folder::Inbox, Mailbox::Inbox)?;
                        true
                    } else {
                        look_new(sh, &mut conn, &st)?
                    };
                    tell(sh, account, watch, &st, "ok", &mut newest_seen);
                    if left == 0 && !came {
                        break;
                    }
                }
                if watch.stopped() {
                    return Ok(());
                }
                if watch.kick.load(Ordering::SeqCst) {
                    continue;
                }
                conn.inbox(false)?;
                if conn.has("IDLE") {
                    conn.idle(IDLE_FOR, &watch.kick)?;
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
        save(&st);
        let Err(fail) = result else { break };
        if watch.stopped() {
            break;
        }
        failures += 1;
        sh.log(&format!("mail {}: {:?} ({} in a row)", account.id, fail, failures));
        watch.set(json!({ "state": "error", "error": fail.json() }));
        sh.push_settings();
        if failures >= 2 {
            show(sh, account, None, None, false);
        }
        // A password said no: not again soon, or the account may be locked.
        let wait = if matches!(fail, Fail::Login(_)) { 300 } else { [15, 30, 60, 120, 300][(failures as usize - 1).min(4)] };
        watch.rest(Duration::from_secs(wait));
    }
}

fn save(st: &Mutex<Store>) {
    st.lock().unwrap().save();
}

// The account's widget and its line in the settings, from the store: the
// unread, the newest unread letter (new mail opens the island, once the
// first sync is done: `newest_seen` known), how much is kept whole.
fn tell(sh: &Shared, account: &Account, watch: &Watch, st: &Mutex<Store>, state: &str, newest_seen: &mut Option<u32>) {
    let (unread, newest, letter, offline) = {
        let s = st.lock().unwrap();
        let unseen: Vec<&Entry> = s.inbox.letters.values().filter(|e| !e.seen).collect();
        let newest = unseen.last().map(|e| e.uid);
        let letter = unseen.last().map(|e| (if e.from.name.is_empty() { e.from.address.clone() } else { e.from.name.clone() }, e.subject.clone()));
        (unseen.len(), newest, letter, s.offline())
    };
    let first = state == "ok" && newest_seen.is_none();
    let is_new = state == "ok" && matches!((newest, *newest_seen), (Some(n), Some(seen)) if n > seen);
    if state == "ok" && (newest > *newest_seen || first) {
        *newest_seen = newest.or(Some(0));
    }
    // Before the first sync, what was kept (nothing said new).
    if state == "ok" || !st.lock().unwrap().inbox.letters.is_empty() {
        show(sh, account, Some(unread), letter.as_ref(), is_new);
    }
    let (have, total, bytes) = offline;
    watch.set(json!({ "state": state, "unread": unread, "offline": { "have": have, "total": total, "bytes": bytes } }));
    sh.push_settings();
}

// A folder made as the server has it (see the top).
fn sync_folder(sh: &Shared, conn: &mut Session, st: &Mutex<Store>, folder: Folder, mailbox: Mailbox<'static>) -> Result<(), Fail> {
    let name = imap::name_of(&mailbox);
    let opened = conn.folder(mailbox, false)?;
    {
        let mut s = st.lock().unwrap();
        let k = s.get(folder);
        if k.validity != opened.validity || k.mailbox != name {
            s.reset(folder, &name, opened.validity);
        }
    }
    let flags = if opened.exists == 0 { Vec::new() } else { conn.flags_all()? };
    let new = st.lock().unwrap().take_flags(folder, &flags);
    heads(sh, conn, st, folder, &new)?;
    let mut s = st.lock().unwrap();
    s.synced[if folder == Folder::Inbox { 0 } else { 1 }] = true;
    Ok(())
}

// Letters not kept yet, by their header fields, a hundred at a time.
fn heads(sh: &Shared, conn: &mut Session, st: &Mutex<Store>, folder: Folder, uids: &[u32]) -> Result<(), Fail> {
    for chunk in uids.chunks(HEADS_AT_ONCE) {
        let set = chunk.iter().map(u32::to_string).collect::<Vec<_>>().join(",");
        let got = conn.heads(&set, true, &FIELDS)?;
        {
            let mut s = st.lock().unwrap();
            for h in got {
                s.add(folder, Entry::from_head(h.uid, (h.seen, h.flagged, h.answered), h.size, h.received, &h.header));
            }
        }
        sh.push_settings();
    }
    Ok(())
}

// The Sent folder, found (SPECIAL-USE, else by its name) and made as the
// server has it; none, and none is kept.
fn sync_sent(sh: &Shared, conn: &mut Session, st: &Mutex<Store>) -> Result<(), Fail> {
    match conn.sent_box()? {
        Some(mailbox) => sync_folder(sh, conn, st, Folder::Sent, mailbox),
        None => {
            let mut s = st.lock().unwrap();
            if !s.sent.letters.is_empty() || !s.sent.mailbox.is_empty() {
                s.reset(Folder::Sent, "", 0);
            }
            s.synced[1] = true;
            Ok(())
        }
    }
}

// Letters kept whole, newest first, for a while (or until stopped or
// kicked); how many are still to come.
fn download(conn: &mut Session, st: &Mutex<Store>, watch: &Watch, skip: &mut HashSet<(Folder, u32)>) -> Result<usize, Fail> {
    let todo: Vec<(Folder, u32)> = st.lock().unwrap().missing().into_iter().filter(|x| !skip.contains(x)).collect();
    let started = Instant::now();
    let mut open: Option<Folder> = None;
    let mut done = 0;
    for &(folder, uid) in &todo {
        if watch.stopped() || watch.kick.load(Ordering::SeqCst) || started.elapsed() > BODIES_FOR {
            break;
        }
        if open != Some(folder) {
            let name = st.lock().unwrap().get(folder).mailbox.clone();
            let Some(mailbox) = imap::mailbox(&name) else { break };
            conn.folder(mailbox, false)?;
            open = Some(folder);
        }
        match conn.letter(uid) {
            Ok(Some((raw, _))) => st.lock().unwrap().keep_body(folder, uid, &raw),
            Ok(None) | Err(Fail::Refused(_)) => {
                skip.insert((folder, uid));
            }
            Err(fail) => return Err(fail),
        }
        done += 1;
    }
    Ok(todo.len() - done)
}

// New mail in the inbox since the newest kept (UID SEARCH UID n:*), by its
// header fields; whether there was any.
fn look_new(sh: &Shared, conn: &mut Session, st: &Mutex<Store>) -> Result<bool, Fail> {
    conn.folder(Mailbox::Inbox, false)?;
    let newest = st.lock().unwrap().inbox.letters.keys().next_back().copied().unwrap_or(0);
    let mut new = conn.newer(newest)?;
    new.sort_unstable_by(|a, b| b.cmp(a));
    heads(sh, conn, st, Folder::Inbox, &new)?;
    Ok(!new.is_empty())
}
