// Shared's locks (shared.rs): a Mutex with a name, which tells when a
// thread takes it while holding another of them. No two of them are to be
// held at once: a thread holding one and waiting on another while a second
// thread does the reverse hangs them both, and the main thread waits on
// them in the menu's callbacks while Tauri's window getters wait on the main
// thread (DEVELOPMENT.md, 踩过的坑: 锁和窗口 getter). Kept to by hand until
// now; here each thread keeps the names of those it holds, in debug builds
// and tests only: a test fails on a second one (a panic), a debug build
// says it once per place in her log and goes on (she runs as the dev debug
// build every day, and a panic would end her), a release build keeps
// nothing and does as a Mutex does.
//
// lock() and try_lock() answer as Mutex's do (a poisoned lock an Err with
// the guard in it), so `.lock().unwrap()` reads as before.
use std::ops::{Deref, DerefMut};
use std::sync::{LockResult, Mutex, MutexGuard, PoisonError, TryLockError, TryLockResult};

pub struct Lock<T> {
    #[cfg_attr(not(debug_assertions), allow(dead_code))]
    name: &'static str,
    inner: Mutex<T>,
}

// Held: the value, and (debugging) the lock's name on this thread's list
// till it is let go. Not Send, as a MutexGuard is not: let go on the thread
// that took it.
pub struct Guard<'a, T> {
    inner: MutexGuard<'a, T>,
    #[cfg(debug_assertions)]
    name: &'static str,
}

impl<T> Lock<T> {
    pub const fn new(name: &'static str, value: T) -> Self {
        Lock { name, inner: Mutex::new(value) }
    }

    // Taken, waiting for it. Said first, should this thread hold another of
    // them: before the wait, which may be for ever.
    #[cfg_attr(debug_assertions, track_caller)]
    pub fn lock(&self) -> LockResult<Guard<'_, T>> {
        #[cfg(debug_assertions)]
        held::taking(self.name, std::panic::Location::caller());
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
        held::took(self.name, std::panic::Location::caller());
        Guard {
            inner,
            #[cfg(debug_assertions)]
            name: self.name,
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

#[cfg(debug_assertions)]
impl<T> Drop for Guard<'_, T> {
    fn drop(&mut self) {
        held::let_go(self.name);
    }
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
        // The locks this thread holds now, by name, with where each was
        // taken, the first first.
        static HELD: RefCell<Vec<(&'static str, At)>> = const { RefCell::new(Vec::new()) };
    }

    pub static SAY: OnceLock<Box<dyn Fn(&str) + Send + Sync>> = OnceLock::new();

    // `name` about to be taken at `at`: another already held here is said.
    pub fn taking(name: &'static str, at: At) {
        let other = HELD.try_with(|h| h.borrow().last().copied()).ok().flatten();
        if let Some((other, since)) = other {
            say(name, at, other, since);
        }
    }

    pub fn took(name: &'static str, at: At) {
        let _ = HELD.try_with(|h| h.borrow_mut().push((name, at)));
    }

    // Let go in any order: the last taken by that name off the list.
    pub fn let_go(name: &'static str) {
        let _ = HELD.try_with(|h| {
            let mut h = h.borrow_mut();
            if let Some(i) = h.iter().rposition(|(n, _)| *n == name) {
                h.remove(i);
            }
        });
    }

    fn say(name: &'static str, at: At, other: &'static str, since: At) {
        let line = format!("lock: {name} taken at {at} while {other} is held (taken at {since}): two of Shared's locks at once");
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
            Some(say) => say(&line),
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
}
