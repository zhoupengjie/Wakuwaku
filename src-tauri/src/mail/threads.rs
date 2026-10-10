// Letters as conversations, threaded the way Thunderbird threads them: by
// what each one answers (In-Reply-To, References), not by its subject. An
// account's inbox and its Sent folder together, so one's own replies are
// in them; a letter in both (one sent to oneself) once, the inbox's. A
// conversation is in the inbox's list when a letter of it is in the inbox.
// Within one, each letter under the one it answers (the nearest it names
// that is there; one whose parent is missing under the one before that it
// names), each in the order sent.
use std::collections::HashMap;

use serde_json::{json, Value};

use super::letters::Filter;
use super::store::{Entry, Folder, Store};
use super::Account;

struct Item<'a> {
    folder: Folder,
    e: &'a Entry,
    // The id it goes by (its Message-ID, else one made up).
    id: String,
}

fn when(e: &Entry) -> i64 {
    e.date.or(e.received.map(|s| s * 1000)).unwrap_or(0)
}

// The groups that answer one another, by union: a letter and every id it
// names are one conversation, even where the letters between are missing.
struct Union {
    up: Vec<usize>,
    of: HashMap<String, usize>,
}

impl Union {
    fn node(&mut self, id: &str) -> usize {
        if let Some(&n) = self.of.get(id) {
            return n;
        }
        let n = self.up.len();
        self.up.push(n);
        self.of.insert(id.to_string(), n);
        n
    }

    fn root(&mut self, mut n: usize) -> usize {
        while self.up[n] != n {
            self.up[n] = self.up[self.up[n]];
            n = self.up[n];
        }
        n
    }

    fn join(&mut self, a: usize, b: usize) {
        let (a, b) = (self.root(a), self.root(b));
        if a != b {
            self.up[b.max(a)] = a.min(b);
        }
    }
}

// A letter for the list: where it is, how deep in its conversation, who
// (oneself said so), when, what about, its flags, the start of its text.
fn letter_view(account: &Account, it: &Item, depth: usize) -> Value {
    let e = it.e;
    let me = it.folder == Folder::Sent || e.from.address.eq_ignore_ascii_case(&account.address);
    json!({
        "account": account.id,
        "folder": it.folder.name(),
        "uid": e.uid,
        "depth": depth,
        "from": if e.from.name.is_empty() { &e.from.address } else { &e.from.name },
        "address": e.from.address,
        "me": me,
        "date": when(e),
        "subject": e.subject,
        "seen": e.seen,
        "flagged": e.flagged,
        "answered": e.answered,
        "attached": e.attached,
        "snippet": e.snippet,
        "messageId": e.id,
    })
}

// An account's conversations that the filter lets through (unread: one
// with a letter in the inbox not read; starred: one with a letter starred),
// unsorted.
pub fn build(account: &Account, store: &Store, filter: Filter) -> Vec<Value> {
    let mut items: Vec<Item> = Vec::new();
    let mut seen_ids: HashMap<&str, ()> = HashMap::new();
    for folder in [Folder::Inbox, Folder::Sent] {
        for e in store.get(folder).letters.values() {
            if !e.id.is_empty() && seen_ids.insert(&e.id, ()).is_some() {
                continue;
            }
            let id = if e.id.is_empty() { format!("{}:{}", folder.name(), e.uid) } else { e.id.clone() };
            items.push(Item { folder, e, id });
        }
    }
    // One conversation: each letter with the ids it names.
    let mut u = Union { up: Vec::new(), of: HashMap::new() };
    let nodes: Vec<usize> = items
        .iter()
        .map(|it| {
            let n = u.node(&it.id);
            for other in it.e.refs.iter().chain(std::iter::once(&it.e.parent)).filter(|x| !x.is_empty()) {
                let m = u.node(other);
                u.join(n, m);
            }
            n
        })
        .collect();
    let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
    for (i, n) in nodes.iter().enumerate() {
        let root = u.root(*n);
        groups.entry(root).or_default().push(i);
    }
    let mut out = Vec::new();
    for members in groups.values() {
        if !members.iter().any(|&i| items[i].folder == Folder::Inbox) {
            continue;
        }
        let passes = match filter {
            Filter::All => true,
            Filter::Unseen => members.iter().any(|&i| items[i].folder == Folder::Inbox && !items[i].e.seen),
            Filter::Flagged => members.iter().any(|&i| items[i].e.flagged),
        };
        if passes {
            out.push(thread_view(account, &items, members));
        }
    }
    out
}

// One conversation: its letters in order, each under the one it answers.
fn thread_view(account: &Account, items: &[Item], members: &[usize]) -> Value {
    let here: HashMap<&str, usize> = members.iter().map(|&i| (items[i].id.as_str(), i)).collect();
    // Each letter's parent: the one it answers, else the nearest before
    // that it names, that is here.
    let parent_of = |i: usize| -> Option<usize> {
        let e = items[i].e;
        std::iter::once(&e.parent).chain(e.refs.iter().rev()).filter(|x| !x.is_empty()).find_map(|x| here.get(x.as_str()).copied()).filter(|&p| p != i)
    };
    let mut parent: HashMap<usize, usize> = HashMap::new();
    for &i in members {
        if let Some(p) = parent_of(i) {
            // Not where it would make a loop.
            let mut at = p;
            let mut steps = 0;
            let loops = loop {
                if at == i {
                    break true;
                }
                match parent.get(&at) {
                    Some(&up) if steps < members.len() => {
                        at = up;
                        steps += 1;
                    }
                    _ => break false,
                }
            };
            if !loops {
                parent.insert(i, p);
            }
        }
    }
    let by_time = |list: &mut Vec<usize>| list.sort_by_key(|&i| (when(items[i].e), items[i].e.uid));
    let mut children: HashMap<usize, Vec<usize>> = HashMap::new();
    let mut roots = Vec::new();
    for &i in members {
        match parent.get(&i) {
            Some(&p) => children.entry(p).or_default().push(i),
            None => roots.push(i),
        }
    }
    by_time(&mut roots);
    for list in children.values_mut() {
        by_time(list);
    }
    // In order, each with how deep it is.
    let mut order: Vec<(usize, usize)> = Vec::new();
    let mut stack: Vec<(usize, usize)> = roots.iter().rev().map(|&r| (r, 0)).collect();
    while let Some((i, depth)) = stack.pop() {
        order.push((i, depth));
        if let Some(kids) = children.get(&i) {
            stack.extend(kids.iter().rev().map(|&k| (k, depth + 1)));
        }
    }
    let letters: Vec<Value> = order.iter().map(|&(i, depth)| letter_view(account, &items[i], depth)).collect();
    let latest = order.iter().map(|&(i, _)| when(items[i].e)).max().unwrap_or(0);
    let unseen = members.iter().filter(|&&i| items[i].folder == Folder::Inbox && !items[i].e.seen).count();
    // Who wrote, in the order they did, each once (oneself as "me").
    let mut people: Vec<Value> = Vec::new();
    let mut who: Vec<String> = Vec::new();
    for l in &letters {
        let key = if l["me"] == true { "me".to_string() } else { l["address"].as_str().unwrap_or("").to_lowercase() };
        if !who.contains(&key) {
            who.push(key);
            people.push(json!({ "name": l["from"], "me": l["me"] }));
        }
    }
    // What a click on it opens: the newest unread in the inbox, else the newest.
    let newest = |unread: bool| order.iter().filter(|&&(i, _)| !unread || (items[i].folder == Folder::Inbox && !items[i].e.seen)).max_by_key(|&&(i, _)| when(items[i].e)).map(|&(i, _)| i);
    let open = newest(true).or_else(|| newest(false)).map(|i| json!({ "folder": items[i].folder.name(), "uid": items[i].e.uid }));
    let first = order.first().map(|&(i, _)| i);
    json!({
        "key": format!("{}|{}", account.id, first.map_or("", |i| items[i].id.as_str())),
        "account": account.id,
        "latest": latest,
        "count": letters.len(),
        "unseen": unseen,
        "flagged": members.iter().any(|&i| items[i].e.flagged),
        "attached": members.iter().any(|&i| items[i].e.attached),
        "subject": first.map_or("", |i| items[i].e.subject.as_str()),
        "people": people,
        "open": open,
        "letters": letters,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mail::store::Person;
    use crate::mail::{Auth, Security, Server};

    fn account() -> Account {
        let server = Server { host: "h".into(), port: 993, security: Security::Ssl, username: "me".into(), auth: Auth::Auto };
        Account { id: "a1".into(), address: "me@example.test".into(), name: String::new(), server, smtp: None, on: true }
    }

    fn letter(uid: u32, from: &str, date: i64, id: &str, parent: &str, refs: &[&str]) -> Entry {
        Entry {
            uid,
            date: Some(date),
            from: Person { name: String::new(), address: from.into() },
            subject: format!("s{uid}"),
            id: id.into(),
            parent: parent.into(),
            refs: refs.iter().map(|r| r.to_string()).collect(),
            seen: true,
            ..Default::default()
        }
    }

    fn store(inbox: Vec<Entry>, sent: Vec<Entry>) -> Store {
        let dir = std::env::temp_dir().join("wakuwaku-threads-unused");
        let mut s = Store::load(dir, &account().server);
        for e in inbox {
            s.inbox.letters.insert(e.uid, e);
        }
        for e in sent {
            s.sent.letters.insert(e.uid, e);
        }
        s
    }

    // (folder, uid, depth) of each letter of each conversation, newest conversation first.
    fn shape(v: &[Value]) -> Vec<Vec<(String, u64, u64)>> {
        let mut v = v.to_vec();
        v.sort_by_key(|t| std::cmp::Reverse(t["latest"].as_i64()));
        v.iter().map(|t| t["letters"].as_array().unwrap().iter().map(|l| (l["folder"].as_str().unwrap().to_string(), l["uid"].as_u64().unwrap(), l["depth"].as_u64().unwrap())).collect()).collect()
    }

    #[test]
    fn replies_go_under_what_they_answer_with_ones_own() {
        let inbox = vec![
            // A contract: c1 (sent by me), c3 missing, c4 answers c3; another reply to c1.
            letter(4, "zs@vendor.example", 400, "c4", "c3", &["c1", "c3"]),
            letter(5, "li@vendor.example", 300, "c5", "c1", &["c1"]),
            // A letter on its own, and one with no Message-ID.
            letter(6, "alice@example.com", 100, "lunch", "", &[]),
            letter(7, "bob@example.com", 50, "", "", &[]),
        ];
        let sent = vec![letter(9, "me@example.test", 200, "c1", "", &[]), letter(10, "me@example.test", 500, "c6", "c4", &["c1", "c3", "c4"])];
        let threads = build(&account(), &store(inbox, sent), Filter::All);
        let s = |f: &str, uid: u64, d: u64| (f.to_string(), uid, d);
        assert_eq!(
            shape(&threads),
            vec![
                vec![s("sent", 9, 0), s("inbox", 5, 1), s("inbox", 4, 1), s("sent", 10, 2)],
                vec![s("inbox", 6, 0)],
                vec![s("inbox", 7, 0)],
            ]
        );
        let contract = threads.iter().find(|t| t["count"] == 4).unwrap();
        assert_eq!(contract["people"].as_array().unwrap().iter().map(|p| p["me"] == true).collect::<Vec<_>>(), [true, false, false]);
        assert_eq!(contract["subject"], "s9");
        assert_eq!(contract["open"], json!({ "folder": "sent", "uid": 10 }));
    }

    #[test]
    fn only_conversations_in_the_inbox_and_as_filtered() {
        let mut unread = letter(4, "zs@vendor.example", 400, "c4", "c1", &["c1"]);
        unread.seen = false;
        let inbox = vec![unread, letter(6, "alice@example.com", 100, "lunch", "", &[])];
        // Sent alone (no answer yet), and a copy of one in the inbox.
        let sent = vec![letter(9, "me@example.test", 200, "c1", "", &[]), letter(11, "me@example.test", 600, "alone", "", &[]), letter(12, "me@example.test", 100, "lunch", "", &[])];
        let st = store(inbox, sent);
        let all = build(&account(), &st, Filter::All);
        assert_eq!(all.len(), 2);
        let lunch = all.iter().find(|t| t["count"] == 1).unwrap();
        assert_eq!(lunch["letters"][0]["folder"], "inbox");
        let unread = build(&account(), &st, Filter::Unseen);
        assert_eq!((unread.len(), unread[0]["unseen"].as_u64(), unread[0]["open"]["uid"].as_u64()), (1, Some(1), Some(4)));
        assert!(build(&account(), &st, Filter::Flagged).is_empty());
    }

    #[test]
    fn letters_that_name_each_other_make_no_loop() {
        let inbox = vec![letter(1, "a@x.test", 100, "x1", "x2", &["x2"]), letter(2, "b@x.test", 200, "x2", "x1", &["x1"])];
        let threads = build(&account(), &store(inbox, vec![]), Filter::All);
        assert_eq!(threads.len(), 1);
        assert_eq!(threads[0]["count"], 2);
    }
}
