// Her locks: a Mutex (Lock) or an RwLock (RwLock) with a name, which tells
// when a thread takes one while holding another. Shared's (shared.rs) and
// every module's own statics are these. A thread holding one and waiting
// on another while a second thread does the reverse hangs them both, and
// the main thread waits on them in the menu's callbacks while Tauri's
// window getters wait on the main thread (DEVELOPMENT.md, 踩过的坑: 锁和窗口
// getter; main_wait.rs). Kept to by hand until 2026-11-11; here each thread
// keeps the names of those it holds, in debug builds and tests only: a test
// fails on a second one (a panic), a debug build says it once per place in
// her log and goes on (she runs as the dev debug build every day, and a
// panic would end her), a release build keeps nothing and does as a Mutex
// does.
//
// Two held at once is allowed only by rank: each lock has one (PLAIN
// unless made with ranked), and a lock may be taken while others are held
// only if its rank is above all of theirs. Locks always taken in one order
// cannot hang each other; a rank is how that order is written down, and a
// pair taken the other way about is said as any other. OUTER, below PLAIN:
// one held around work that takes ordinary locks (taken first, with none
// held), never on the main thread, so it may be held while that work
// waits for the main thread (none_held lets it be; taking one on the main
// thread is said). LEAF, above it: one taken inside ordinary ones, with
// nothing inside it.
//
// lock() and try_lock() answer as Mutex's do (a poisoned lock an Err with
// the guard in it), so `.lock().unwrap()` reads as before; a Condvar waits
// on a held Guard (wait_while, wait_timeout_while).
use std::mem::ManuallyDrop;
use std::ops::{Deref, DerefMut};
use std::sync::{Condvar, LockResult, Mutex, MutexGuard, PoisonError, TryLockError, TryLockResult, WaitTimeoutResult};
use std::time::Duration;

// One held around a whole job that takes other locks (an install, an
// account's edit, a catalog's read): taken with none held, and never on
// the main thread.
pub const OUTER: u8 = 50;
// The rank of a lock made with new: two of them are never held at once.
pub const PLAIN: u8 = 100;
// One taken inside others, with nothing taken inside it: a list of lines
// for the log, a count.
pub const LEAF: u8 = 200;

// The main thread (main_wait.rs here, as she starts): an OUTER lock is
// never taken there.
static MAIN: std::sync::OnceLock<std::thread::ThreadId> = std::sync::OnceLock::new();

pub fn main_is_this_thread() {
    let _ = MAIN.set(std::thread::current().id());
}

pub fn on_main() -> bool {
    MAIN.get().is_some_and(|id| *id == std::thread::current().id())
}

pub struct Lock<T> {
    #[cfg_attr(not(debug_assertions), allow(dead_code))]
    name: &'static str,
    #[cfg_attr(not(debug_assertions), allow(dead_code))]
    rank: u8,
    inner: Mutex<T>,
}

// Held: the value, and (debugging) the lock's name on this thread's list
// till it is let go. Not Send, as a MutexGuard is not: let go on the thread
// that took it.
pub struct Guard<'a, T> {
    inner: MutexGuard<'a, T>,
    #[cfg(debug_assertions)]
    name: &'static str,
    #[cfg(debug_assertions)]
    rank: u8,
}

impl<T> Lock<T> {
    // Unique by name across her code (tests::names_are_unique): the name is
    // what her log says, and what a thread's list goes by.
    pub const fn new(name: &'static str, value: T) -> Self {
        Lock { name, rank: PLAIN, inner: Mutex::new(value) }
    }

    // One that may be taken while locks of lower rank are held (and none of
    // its own rank or above): see the top of this file.
    #[allow(dead_code)]
    pub const fn ranked(name: &'static str, rank: u8, value: T) -> Self {
        Lock { name, rank, inner: Mutex::new(value) }
    }

    // Taken, waiting for it. Said first, should this thread hold another of
    // them: before the wait, which may be for ever.
    #[cfg_attr(debug_assertions, track_caller)]
    pub fn lock(&self) -> LockResult<Guard<'_, T>> {
        #[cfg(debug_assertions)]
        held::taking(self.name, self.rank, std::panic::Location::caller());
        match self.inner.lock() {
            Ok(inner) => Ok(self.held(inner)),
            Err(poisoned) => Err(PoisonError::new(self.held(poisoned.into_inner()))),
        }
    }

    // Taken only if it is free now. Never waits, so never hangs anyone;
    // held, it counts as any other.
    #[cfg_attr(debug_assertions, track_caller)]
    pub fn try_lock(&self) -> TryLockResult<Guard<'_, T>> {
        match self.inner.try_lock() {
            Ok(inner) => Ok(self.held(inner)),
            Err(TryLockError::Poisoned(poisoned)) => Err(TryLockError::Poisoned(PoisonError::new(self.held(poisoned.into_inner())))),
            Err(TryLockError::WouldBlock) => Err(TryLockError::WouldBlock),
        }
    }

    // On this thread's list, with where it was taken (the caller of lock or
    // try_lock: track_caller carries it through).
    #[cfg_attr(debug_assertions, track_caller)]
    fn held<'a>(&'a self, inner: MutexGuard<'a, T>) -> Guard<'a, T> {
        #[cfg(debug_assertions)]
        held::took(self.name, self.rank, std::panic::Location::caller());
        Guard {
            inner,
            #[cfg(debug_assertions)]
            name: self.name,
            #[cfg(debug_assertions)]
            rank: self.rank,
        }
    }
}

impl<'a, T> Guard<'a, T> {
    // Waits on `cv` while `cond` holds, as Condvar::wait_while does: the lock
    // let go meanwhile (off this thread's list) and taken again after (said,
    // should this thread hold another then: waiting with one held is
    // waiting for it as much as lock() is).
    #[cfg_attr(debug_assertions, track_caller)]
    #[allow(dead_code)]
    pub fn wait_while(self, cv: &Condvar, cond: impl FnMut(&mut T) -> bool) -> LockResult<Guard<'a, T>> {
        let (inner, back) = self.let_go_for_wait();
        match cv.wait_while(inner, cond) {
            Ok(inner) => Ok(back.again(inner)),
            Err(poisoned) => Err(PoisonError::new(back.again(poisoned.into_inner()))),
        }
    }

    // The same with a time limit, as Condvar::wait_timeout_while does.
    #[cfg_attr(debug_assertions, track_caller)]
    pub fn wait_timeout_while(self, cv: &Condvar, dur: Duration, cond: impl FnMut(&mut T) -> bool) -> LockResult<(Guard<'a, T>, WaitTimeoutResult)> {
        let (inner, back) = self.let_go_for_wait();
        match cv.wait_timeout_while(inner, dur, cond) {
            Ok((inner, timeout)) => Ok((back.again(inner), timeout)),
            Err(poisoned) => {
                let (inner, timeout) = poisoned.into_inner();
                Err(PoisonError::new((back.again(inner), timeout)))
            }
        }
    }

    // The MutexGuard out for the Condvar, and this lock off the list.
    #[cfg_attr(debug_assertions, track_caller)]
    fn let_go_for_wait(self) -> (MutexGuard<'a, T>, Back) {
        let me = ManuallyDrop::new(self);
        // SAFETY: `me` is never dropped nor used again, so the guard is
        // moved out of it exactly once.
        let inner = unsafe { std::ptr::read(&me.inner) };
        #[cfg(debug_assertions)]
        {
            held::let_go(me.name);
            held::taking(me.name, me.rank, std::panic::Location::caller());
        }
        let back = Back {
            #[cfg(debug_assertions)]
            name: me.name,
            #[cfg(debug_assertions)]
            rank: me.rank,
            #[cfg(debug_assertions)]
            at: std::panic::Location::caller(),
        };
        (inner, back)
    }
}

// How a Guard let go for a Condvar's wait is put back on the list.
struct Back {
    #[cfg(debug_assertions)]
    name: &'static str,
    #[cfg(debug_assertions)]
    rank: u8,
    #[cfg(debug_assertions)]
    at: &'static std::panic::Location<'static>,
}

impl Back {
    fn again<T>(self, inner: MutexGuard<'_, T>) -> Guard<'_, T> {
        #[cfg(debug_assertions)]
        held::took(self.name, self.rank, self.at);
        Guard {
            inner,
            #[cfg(debug_assertions)]
            name: self.name,
            #[cfg(debug_assertions)]
            rank: self.rank,
        }
    }
}

impl<T> Deref for Guard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.inner
    }
}

impl<T> DerefMut for Guard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.inner
    }
}

impl<T: std::fmt::Display> std::fmt::Display for Guard<'_, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        (**self).fmt(f)
    }
}

#[cfg(debug_assertions)]
impl<T> Drop for Guard<'_, T> {
    fn drop(&mut self) {
        held::let_go(self.name);
    }
}

// An RwLock with a name, said as a Lock is: a read held counts as any lock
// held (a second read of the same one on a thread can hang behind a writer
// waiting between them, as std's may).
pub struct RwLock<T> {
    #[cfg_attr(not(debug_assertions), allow(dead_code))]
    name: &'static str,
    #[cfg_attr(not(debug_assertions), allow(dead_code))]
    rank: u8,
    inner: std::sync::RwLock<T>,
}

pub struct ReadGuard<'a, T> {
    inner: std::sync::RwLockReadGuard<'a, T>,
    #[cfg(debug_assertions)]
    name: &'static str,
}

pub struct WriteGuard<'a, T> {
    inner: std::sync::RwLockWriteGuard<'a, T>,
    #[cfg(debug_assertions)]
    name: &'static str,
}

impl<T> RwLock<T> {
    pub const fn new(name: &'static str, value: T) -> Self {
        RwLock { name, rank: PLAIN, inner: std::sync::RwLock::new(value) }
    }

    #[cfg_attr(debug_assertions, track_caller)]
    pub fn read(&self) -> LockResult<ReadGuard<'_, T>> {
        #[cfg(debug_assertions)]
        held::taking(self.name, self.rank, std::panic::Location::caller());
        let held = |inner| {
            #[cfg(debug_assertions)]
            held::took(self.name, self.rank, std::panic::Location::caller());
            ReadGuard {
                inner,
                #[cfg(debug_assertions)]
                name: self.name,
            }
        };
        match self.inner.read() {
            Ok(inner) => Ok(held(inner)),
            Err(poisoned) => Err(PoisonError::new(held(poisoned.into_inner()))),
        }
    }

    #[cfg_attr(debug_assertions, track_caller)]
    pub fn write(&self) -> LockResult<WriteGuard<'_, T>> {
        #[cfg(debug_assertions)]
        held::taking(self.name, self.rank, std::panic::Location::caller());
        let held = |inner| {
            #[cfg(debug_assertions)]
            held::took(self.name, self.rank, std::panic::Location::caller());
            WriteGuard {
                inner,
                #[cfg(debug_assertions)]
                name: self.name,
            }
        };
        match self.inner.write() {
            Ok(inner) => Ok(held(inner)),
            Err(poisoned) => Err(PoisonError::new(held(poisoned.into_inner()))),
        }
    }
}

impl<T> Deref for ReadGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.inner
    }
}

impl<T> Deref for WriteGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.inner
    }
}

impl<T> DerefMut for WriteGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.inner
    }
}

#[cfg(debug_assertions)]
impl<T> Drop for ReadGuard<'_, T> {
    fn drop(&mut self) {
        held::let_go(self.name);
    }
}

#[cfg(debug_assertions)]
impl<T> Drop for WriteGuard<'_, T> {
    fn drop(&mut self) {
        held::let_go(self.name);
    }
}

// About to wait for something that may wait on any of her locks (the main
// thread, mostly: main_wait.rs): none is to be held here, an OUTER one
// aside (never taken on the main thread). Said as two locks are (a test
// fails, a debug build says it once per place).
#[cfg_attr(debug_assertions, track_caller)]
pub fn none_held(what: &str) {
    #[cfg(debug_assertions)]
    held::none(what, std::panic::Location::caller());
    #[cfg(not(debug_assertions))]
    let _ = what;
}

// Where a second one held is said in a debug build: her log, once she is
// up (Shared::new); before that, on the console the debug build has.
#[cfg(debug_assertions)]
pub fn say_with(say: impl Fn(&str) + Send + Sync + 'static) {
    let _ = held::SAY.set(Box::new(say));
}

#[cfg(not(debug_assertions))]
pub fn say_with(_say: impl Fn(&str) + Send + Sync + 'static) {}

#[cfg(debug_assertions)]
mod held {
    use std::cell::RefCell;
    use std::collections::HashSet;
    use std::panic::Location;
    use std::sync::{Mutex, OnceLock};

    type At = &'static Location<'static>;

    thread_local! {
        // The locks this thread holds now, by name, with each one's rank and
        // where it was taken, the first first.
        static HELD: RefCell<Vec<(&'static str, u8, At)>> = const { RefCell::new(Vec::new()) };
    }

    pub static SAY: OnceLock<Box<dyn Fn(&str) + Send + Sync>> = OnceLock::new();

    // `name` about to be taken at `at`: another already held here, of its
    // rank or above, is said (the highest, else the last taken).
    pub fn taking(name: &'static str, rank: u8, at: At) {
        let other = HELD.try_with(|h| h.borrow().iter().rev().find(|(_, r, _)| *r >= rank).copied()).ok().flatten();
        if let Some((other, _, since)) = other {
            say(&format!("lock: {name} taken at {at} while {other} is held (taken at {since}): two locks at once"), at, since);
        }
        if rank <= super::OUTER && super::on_main() {
            say(&format!("lock: {name} taken at {at} on the main thread: an outer lock is held while the main thread is waited for"), at, at);
        }
    }

    pub fn took(name: &'static str, rank: u8, at: At) {
        let _ = HELD.try_with(|h| h.borrow_mut().push((name, rank, at)));
    }

    // Let go in any order: the last taken by that name off the list.
    pub fn let_go(name: &'static str) {
        let _ = HELD.try_with(|h| {
            let mut h = h.borrow_mut();
            if let Some(i) = h.iter().rposition(|(n, _, _)| *n == name) {
                h.remove(i);
            }
        });
    }

    // About to wait at `at` with none to be held (an outer one aside).
    pub fn none(what: &str, at: At) {
        let other = HELD.try_with(|h| h.borrow().iter().rev().find(|(_, r, _)| *r > super::OUTER).copied()).ok().flatten();
        if let Some((other, _, since)) = other {
            say(&format!("lock: {what} at {at} waits while {other} is held (taken at {since})"), at, since);
        }
    }

    fn say(line: &str, at: At, since: At) {
        if cfg!(test) {
            panic!("{line}");
        }
        // Once for each two places: a loop would say it at every turn.
        static SAID: Mutex<Option<HashSet<(At, At)>>> = Mutex::new(None);
        let first = SAID.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashSet::new).insert((at, since));
        if !first {
            return;
        }
        match SAY.get() {
            Some(say) => say(line),
            None => eprintln!("wakuwaku: {line}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_after_another_is_fine() {
        let (a, b) = (Lock::new("a", 1), Lock::new("b", 2));
        let x = *a.lock().unwrap();
        let y = *b.lock().unwrap();
        {
            let mut a = a.lock().unwrap();
            *a += 10;
        }
        let a = *a.lock().unwrap();
        let b = *b.lock().unwrap();
        assert_eq!((x, y, a, b), (1, 2, 11, 2));
    }

    #[test]
    #[should_panic(expected = "lock: b taken at")]
    fn two_at_once_fail_a_test() {
        let (a, b) = (Lock::new("a", 1), Lock::new("b", 2));
        let _a = a.lock().unwrap();
        let _b = b.lock().unwrap();
    }

    // A temporary's lock lives to the end of its statement: two in one
    // expression are two at once.
    #[test]
    #[should_panic(expected = "while a is held")]
    fn two_in_one_statement_are_two_at_once() {
        let (a, b) = (Lock::new("a", 1), Lock::new("b", 2));
        let _sum = *a.lock().unwrap() + *b.lock().unwrap();
    }

    // Let go in another order than taken (the second taken by try_lock,
    // which never waits and so is not said): nothing is left on the list,
    // and each may be taken again.
    #[test]
    fn let_go_in_any_order() {
        let (a, b) = (Lock::new("a", 1), Lock::new("b", 2));
        let ga = a.lock().unwrap();
        let gb = b.try_lock().unwrap();
        drop(ga);
        drop(gb);
        let x = *b.lock().unwrap();
        let y = *a.lock().unwrap();
        assert_eq!(x + y, 3);
    }

    #[test]
    fn try_lock_answers_as_a_mutex_does() {
        let a = Lock::new("a", 1);
        let held = a.lock().unwrap();
        assert!(matches!(a.try_lock(), Err(TryLockError::WouldBlock)));
        drop(held);
        assert_eq!(a.try_lock().map(|g| *g).unwrap_or(0), 1);
    }

    // Poisoned by a panic while held: an Err with the guard, as Mutex's.
    #[test]
    fn a_poisoned_lock_is_an_err_with_its_value() {
        let a = std::sync::Arc::new(Lock::new("a", 5));
        let b = a.clone();
        let _ = std::thread::spawn(move || {
            let _g = b.lock().unwrap();
            panic!("held");
        })
        .join();
        let err = a.lock().err().expect("poisoned");
        assert_eq!(*err.into_inner(), 5);
        // Let go again, and nothing left held on this thread.
        let _again = a.lock().unwrap_or_else(|e| e.into_inner());
    }

    // Each thread its own list: one thread's held lock is nothing to another.
    #[test]
    fn another_threads_lock_is_not_this_ones() {
        let a = std::sync::Arc::new(Lock::new("a", 1));
        let b = std::sync::Arc::new(Lock::new("b", 2));
        let _a = a.lock().unwrap();
        let b2 = b.clone();
        assert_eq!(std::thread::spawn(move || *b2.lock().unwrap()).join().unwrap(), 2);
    }

    // Ranked: a higher one may be taken with a lower one held, not the
    // other way about, and not two of one rank.
    #[test]
    fn up_the_ranks_is_fine() {
        let (low, high) = (Lock::ranked("low", 1, 1), Lock::ranked("high", 2, 2));
        let _l = low.lock().unwrap();
        let h = *high.lock().unwrap();
        assert_eq!(h, 2);
    }

    #[test]
    #[should_panic(expected = "lock: low taken at")]
    fn down_the_ranks_fails_a_test() {
        let (low, high) = (Lock::ranked("low", 1, 1), Lock::ranked("high", 2, 2));
        let _h = high.lock().unwrap();
        let _l = low.lock().unwrap();
    }

    // Each held lock counts, not only the last taken: below one of them is
    // said even when above the last.
    #[test]
    #[should_panic(expected = "while top is held")]
    fn every_held_lock_counts() {
        let (top, low, mid) = (Lock::ranked("top", 3, 0), Lock::ranked("low", 1, 0), Lock::ranked("mid", 2, 0));
        let _t = top.lock().unwrap();
        let _l = low.try_lock().unwrap();
        let _m = mid.lock().unwrap();
    }

    // A Condvar's wait lets the lock go meanwhile: another thread takes it
    // and wakes this one, and after it the lock is held (and let go) again.
    #[test]
    fn a_condvar_waits_on_a_guard() {
        use std::sync::Arc;
        let pair = Arc::new((Lock::new("flag", false), Condvar::new()));
        let other = pair.clone();
        let waker = std::thread::spawn(move || {
            *other.0.lock().unwrap() = true;
            other.1.notify_all();
        });
        let (guard, _) = pair.0.lock().unwrap().wait_timeout_while(&pair.1, Duration::from_secs(10), |set| !*set).unwrap();
        assert!(*guard);
        drop(guard);
        waker.join().unwrap();
        let again = pair.0.lock().unwrap().wait_while(&pair.1, |set| !*set).map(|g| *g).unwrap();
        assert!(again);
        // Let go after the wait: another may be taken.
        let other = Lock::new("other", 0);
        assert_eq!(*other.lock().unwrap(), 0);
    }

    // Waiting on a Condvar while holding another lock is said.
    #[test]
    #[should_panic(expected = "lock: flag taken at")]
    fn waiting_with_another_held_is_said() {
        let other = Lock::new("other", 0);
        let flag = Lock::new("flag", true);
        let cv = Condvar::new();
        let g = flag.lock().unwrap();
        let _o = other.try_lock().unwrap();
        let _ = g.wait_timeout_while(&cv, Duration::from_millis(1), |set| !*set);
    }

    #[test]
    fn reads_and_writes_one_after_another() {
        let rw = RwLock::new("rw", 1);
        let r = *rw.read().unwrap();
        *rw.write().unwrap() += r;
        assert_eq!(*rw.read().unwrap(), 2);
    }

    #[test]
    #[should_panic(expected = "lock: rw taken at")]
    fn a_read_inside_a_lock_is_said() {
        let (a, rw) = (Lock::new("a", 1), RwLock::new("rw", 2));
        let _a = a.lock().unwrap();
        let _r = rw.read().unwrap();
    }

    #[test]
    fn none_held_with_none_held_is_fine() {
        none_held("a getter");
        let a = Lock::new("a", 1);
        drop(a.lock().unwrap());
        none_held("a getter");
    }

    #[test]
    #[should_panic(expected = "lock: a getter at")]
    fn none_held_with_one_held_fails_a_test() {
        let a = Lock::new("a", 1);
        let _a = a.lock().unwrap();
        none_held("a getter");
    }

    // An outer lock may be held over a wait for the main thread (it is
    // never taken there), and ordinary ones taken inside it.
    #[test]
    fn an_outer_lock_is_held_around_ordinary_ones_and_waits() {
        let (job, a) = (Lock::ranked("job", OUTER, ()), Lock::new("a", 1));
        let _job = job.lock().unwrap();
        none_held("a getter");
        assert_eq!(*a.lock().unwrap(), 1);
    }

    // The main thread taking an outer one: said (here, on a thread named
    // main for the test).
    #[test]
    fn an_outer_lock_on_the_main_thread_is_said() {
        let said = std::thread::spawn(|| {
            main_is_this_thread();
            let job = Lock::ranked("job", OUTER, ());
            let taken = std::panic::catch_unwind(|| drop(job.lock()));
            taken.err().and_then(|e| e.downcast_ref::<String>().cloned()).unwrap_or_default()
        })
        .join()
        .unwrap();
        assert!(said.contains("on the main thread"), "{said}");
    }

    // Each lock's name once in her code: a name is what the log says and
    // what a thread's list goes by. Read from the sources (this file's own
    // tests aside).
    #[test]
    fn names_are_unique() {
        fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            for e in std::fs::read_dir(dir).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, out);
                } else if p.extension().is_some_and(|x| x == "rs") && !p.ends_with("lock.rs") {
                    out.push(p);
                }
            }
        }
        let mut files = Vec::new();
        walk(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut files);
        let mut seen: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        let mut twice = Vec::new();
        for f in files {
            let text = std::fs::read_to_string(&f).unwrap();
            for opener in ["Lock::new(\"", "Lock::ranked(\""] {
                for (i, _) in text.match_indices(opener) {
                    let rest = &text[i + opener.len()..];
                    let name = &rest[..rest.find('"').unwrap()];
                    let at = format!("{}", f.display());
                    if let Some(first) = seen.insert(name.to_string(), at.clone()) {
                        twice.push(format!("{name}: {first} and {at}"));
                    }
                }
            }
        }
        assert!(seen.len() > 100, "found only {} lock names", seen.len());
        assert!(twice.is_empty(), "lock names used twice: {twice:?}");
    }
}
