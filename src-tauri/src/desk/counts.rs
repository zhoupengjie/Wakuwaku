// What the desktop's windows do, counted each minute for the debug log.
use super::*;

// Each minute while there are desktop windows and the debug log is on
// (loops.rs ticker): the pointer's looks, the words pushed and sent, the
// events to the pages, what the pages asked, the windows put back in their
// place, the looks at whether a desktop is covered, the desk thread's
// messages; and, with WAKUWAKU_DESK_TRACE=1, every message the main thread
// took, by kind (what wakes it).
pub(super) static POLLS: AtomicU64 = AtomicU64::new(0);
pub(super) static PUSHES: AtomicU64 = AtomicU64::new(0);
pub(super) static SENT: AtomicU64 = AtomicU64::new(0);
pub(super) static EMITS: AtomicU64 = AtomicU64::new(0);
pub(super) static COMMANDS: AtomicU64 = AtomicU64::new(0);
pub(super) static PLACED: AtomicU64 = AtomicU64::new(0);
pub(super) static LOOKS: AtomicU64 = AtomicU64::new(0);
pub(super) static THREAD_MSGS: AtomicU64 = AtomicU64::new(0);
pub(super) static TRACE: Lock<Option<Vec<(u32, bool, bool, u64)>>> = Lock::new("desk::counts::TRACE", None);

pub(super) fn count(c: &AtomicU64) {
    c.fetch_add(1, Ordering::Relaxed);
}

pub(super) fn emit(sh: &Shared, label: &str, event: &str, payload: Value) {
    count(&EMITS);
    let _ = sh.app.emit_to(label, event, payload);
}

// The main thread's messages counted (WAKUWAKU_DESK_TRACE=1), once.
pub(super) fn trace_main(sh: &Arc<Shared>) {
    if std::env::var_os("WAKUWAKU_DESK_TRACE").is_none() {
        return;
    }
    {
        let mut trace = TRACE.lock().unwrap();
        if trace.is_some() {
            return;
        }
        *trace = Some(Vec::new());
    }
    let _ = sh.app.run_on_main_thread(imp::trace_this_thread);
    sh.log("desk: the main thread's messages counted (WAKUWAKU_DESK_TRACE)");
}

// One message the main thread took: by its kind, to a window or to the
// thread itself (another thread's wake: tao's proxy), posted or sent.
pub(super) fn traced(hwnd: isize, msg: u32, sent: bool) {
    let Ok(mut trace) = TRACE.try_lock() else { return };
    let Some(list) = trace.as_mut() else { return };
    let thread = hwnd == 0;
    match list.iter_mut().find(|(m, t, s, _)| *m == msg && *t == thread && *s == sent) {
        Some(e) => e.3 += 1,
        None => list.push((msg, thread, sent, 1)),
    }
}

pub fn minute(sh: &Shared) {
    let take = |c: &AtomicU64| c.swap(0, Ordering::Relaxed);
    let counts = [take(&POLLS), take(&PUSHES), take(&SENT), take(&EMITS), take(&COMMANDS), take(&PLACED), take(&LOOKS), take(&THREAD_MSGS)];
    let traced = TRACE.lock().unwrap().as_mut().map(std::mem::take);
    if !any() || !sh.is_logging() {
        return;
    }
    let [polls, pushes, sent, emits, commands, placed, looks, thread] = counts;
    let mut line = format!("desk: a minute: {polls} looks at the pointer, {pushes} pushes ({sent} sent), {emits} events, {commands} asked, {placed} put back, {looks} covered-looks, {thread} desk-thread messages");
    if let Some(mut list) = traced {
        list.sort_by(|a, b| b.3.cmp(&a.3));
        let total: u64 = list.iter().map(|e| e.3).sum();
        let top: Vec<String> = list.iter().take(8).map(|(m, t, s, n)| format!("{m:#06x}{}{}x{n}", if *t { " thread" } else { "" }, if *s { " sent" } else { "" })).collect();
        line.push_str(&format!("; main thread {total} messages: {}", top.join(", ")));
    }
    sh.log(&line);
}

