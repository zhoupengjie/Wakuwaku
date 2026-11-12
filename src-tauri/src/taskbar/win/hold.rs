// Windows' own taskbar put away while hers is up, and given back
// (shell.rs): the strip taken (take), given back (give_back); another copy of
// her holding it, waited for; Explorer starting, or started again over and
// over, waited for before anything is taken (zhoupengjie/Wakuwaku#85); its
// taskbar shown again and put away again, and given up to another program
// that will not stop showing it (the tug, #69); its picture for the
// handover (strip_picture).
use super::*;
use crate::lock::Lock;

pub(super) struct Host {
    pub(super) hwnd: isize,
    pub(super) thread: JoinHandle<()>,
}

pub(super) static HOST: Lock<Option<Host>> = Lock::new("taskbar::win::hold::HOST", None);
// The buttons' thread (buttons.rs tasks_loop).
pub(super) static TASKS: Lock<Option<JoinHandle<()>>> = Lock::new("taskbar::win::hold::TASKS", None);
// The keyboard's thread (keys.rs keys_loop).
pub(super) static KEYS: Lock<Option<JoinHandle<()>>> = Lock::new("taskbar::win::hold::KEYS", None);

// The edge her strip is along: Windows' own taskbar keeps its room along
// the bottom for a bottom strip to lie over, and hides itself (keeping
// none) for another edge, where an empty band would be left at the
// bottom (shell.rs keep_room), but for her dock kept in that room (room:
// split, splitDock room); put away again at once if it is up.
pub fn set_edge(edge: Edge, room: bool) {
    // Back to the bottom from another edge: the desktop's icons where
    // they are, before Windows' taskbar takes its room back and the
    // desktop moves them (icons.rs hold).
    if edge == Edge::Bottom && *EDGE.lock().unwrap() != Edge::Bottom && is_up() {
        if let Some(sh) = sh() {
            for line in crate::icons::hold_within(&sh.dir, Duration::from_secs(2)) {
                sh.log(&line);
            }
        }
    }
    let was = std::mem::replace(&mut *EDGE.lock().unwrap(), edge);
    let kept = shell::kept_room();
    shell::keep_room(room);
    if (was != edge || kept != room) && is_up() {
        log(&format!("the strip now along the {edge:?}: Windows' kept room {}", if room { "kept" } else { "let go" }));
        shell::rehide();
    }
}

pub fn is_up() -> bool {
    HOST.lock().unwrap().is_some()
}

// Quitting with the handover: Windows' taskbar shown again under hers on
// purpose (island/ handoff_out), so neither the timer nor its show
// event puts it away again in the second before it is given back (it
// was: the foot of the screen bare for that second).
pub(super) static HANDING_BACK: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn handing_back() {
    HANDING_BACK.store(true, Ordering::SeqCst);
}

// The main display's work area's bottom (physical), for the log.
pub fn work_area_bottom() -> i32 {
    work_bottom()
}

// The main display's work area (physical: left, top, right, bottom), for the log.
pub fn work_area_rect() -> (i32, i32, i32, i32) {
    let r = work_area();
    (r.left, r.top, r.right, r.bottom)
}

// The room Windows' own taskbar, put away, keeps along the bottom of the
// display (physical): the strip goes over it, with no bar of ours (that
// would stack above it). Waited for a moment: one set to hide itself
// until just now takes its room after a slide. None: no room there (its
// taskbar on another edge or display); a bar of ours then.
pub fn explorer_room(mon: (i32, i32, i32, i32)) -> Option<(i32, i32, i32, i32)> {
    let t = Instant::now();
    loop {
        let room = shell::room(mon);
        if room.is_some() || t.elapsed() > Duration::from_millis(1500) {
            return room;
        }
        std::thread::sleep(Duration::from_millis(30));
    }
}

// Another copy of her (another port, its own data) holding the taskbar
// now, by its host window: its pid, and when it started. Two holding it
// at once hand the tray's place back and forth twice a second, and her
// menus change under the pointer (2026-10-10: a session's hook started
// the live copy while a test copy held it).
#[link(name = "user32")]
extern "system" {
    pub(super) fn FindWindowExW(parent: Hwnd, after: Hwnd, class: *const u16, title: *const u16) -> Hwnd;
    pub(super) fn GetWindowThreadProcessId(hwnd: Hwnd, pid: *mut u32) -> u32;
}

pub(super) static WAITING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub(super) fn other_holder() -> Option<(u32, Option<u64>)> {
    let class = wide("WakuwakuTaskbarHost");
    let me = std::process::id();
    let mut at: Hwnd = std::ptr::null_mut();
    loop {
        // SAFETY: a class name of our own; walking the top-level windows.
        at = unsafe { FindWindowExW(std::ptr::null_mut(), at, class.as_ptr(), std::ptr::null()) };
        if at.is_null() {
            return None;
        }
        let mut pid = 0;
        // SAFETY: our own out-parameter.
        unsafe { GetWindowThreadProcessId(at, &mut pid) };
        if pid != 0 && pid != me {
            return Some((pid, shell::started(pid)));
        }
    }
}

// Waiting for another copy to let the taskbar go, or for Explorer to
// settle (wait_for_explorer): this one's window stays hidden (island/
// is_shown) and takes nothing.
pub fn waiting() -> bool {
    WAITING.load(Ordering::SeqCst)
}

// Waits, looking every two seconds; the taskbar taken once the other has
// gone (its window shown again, which takes the strip).
pub(super) fn wait_for_other(sh: &Shared, pid: u32) {
    if WAITING.swap(true, Ordering::SeqCst) {
        return;
    }
    sh.log(&format!("taskbar: another copy of her (pid {pid}) holds the taskbar: this one waits"));
    let sh = crate::shared(&sh.app);
    std::thread::spawn(move || {
        while other_holder().is_some() {
            std::thread::sleep(Duration::from_secs(2));
        }
        WAITING.store(false, Ordering::SeqCst);
        sh.log("taskbar: the other copy let the taskbar go: this one takes it");
        crate::island::apply_visibility(&sh);
    });
}

// --- Explorer starting (zhoupengjie/Wakuwaku#85) ---------------------------------
// On 22621 (23H2 22631.2861, explorer.exe 10.0.22621.2792) Explorer
// failed fast again and again while it was starting with hers the first
// tray (0xc0000409 in ucrtbase.dll, as in #62): at a logon her task had
// started her before Explorer had a taskbar at all, or a few seconds
// after, and each Explorer Windows started again found hers first again,
// some 25 a minute, until Windows stopped starting it: no desktop, no
// taskbar. Started after Explorer had settled (by the Run value), none.
// So she takes the taskbar only once Explorer has a taskbar and a
// desktop and has run EXPLORER_SETTLE (its battery icon, the last of
// #85's log before each crash, came some 6 s after its taskbar); while
// she holds it, a new Explorer has hers set behind its tray until it has
// run as long (explorer_check, come_forward); and Explorer started
// again LOOP_TIMES times within LOOP_WITHIN all the same, the taskbar is
// given back for a while (step_away), the home kept.
pub(super) const EXPLORER_SETTLE: Duration = Duration::from_secs(30);
// An Explorer that never comes (another shell, Explorer ended by hand):
// the taskbar taken anyway after so long, and not waited for again.
pub(super) const EXPLORER_AT_MOST: Duration = Duration::from_secs(90);
pub(super) static EXPLORER_FORCED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub(super) const LOOP_TIMES: usize = 3;
pub(super) const LOOP_WITHIN: Duration = Duration::from_secs(120);
// Given back for Explorer to run so long first, then twice as long each
// time it loops again, up to the most.
pub(super) const AWAY_FIRST: u64 = 120;
pub(super) const AWAY_MOST: u64 = 1800;

// What keeps her from taking the taskbar from Explorer yet, for the log
// (None: nothing): how long Explorer's taskbar's process has run (None:
// it has none), whether the desktop is Explorer's, how long it is to run.
pub(super) fn unready(up: Option<Duration>, desktop: bool, settle: Duration) -> Option<String> {
    match up {
        None => Some("Explorer has no taskbar yet".to_string()),
        Some(_) if !desktop => Some("Explorer's desktop is not up yet".to_string()),
        Some(up) if up < settle => Some(format!("Explorer up {:.1} s, under the {} s it is given to start", up.as_secs_f64(), settle.as_secs())),
        Some(_) => None,
    }
}

pub(super) fn explorer_up(pid: u32) -> Option<Duration> {
    (pid != 0).then(|| shell::running_for(pid)).flatten()
}

pub(super) fn explorer_unready(settle: Duration) -> Option<String> {
    if EXPLORER_FORCED.load(Ordering::SeqCst) {
        return None;
    }
    unready(explorer_up(shell::explorer_pid()), shell::desktop_up(), settle)
}

// Waits as for another copy (waiting: her window hidden, nothing taken),
// looking every second, until Explorer has run `settle` (at_most: or so
// long has gone by); then shown again, which takes the strip.
pub(super) fn wait_for_explorer(sh: &Shared, why: &str, settle: Duration, at_most: Option<Duration>) {
    if WAITING.swap(true, Ordering::SeqCst) {
        return;
    }
    sh.log(&format!("taskbar: {why}: this one waits for it (zhoupengjie/Wakuwaku#85)"));
    let sh = crate::shared(&sh.app);
    std::thread::spawn(move || {
        let began = Instant::now();
        loop {
            std::thread::sleep(Duration::from_secs(1));
            let Some(why) = explorer_unready(settle) else {
                sh.log(&format!("taskbar: Explorer has run {} s, waited {:.0} s: this one takes it", settle.as_secs(), began.elapsed().as_secs_f64()));
                break;
            };
            if at_most.is_some_and(|most| began.elapsed() >= most) {
                EXPLORER_FORCED.store(true, Ordering::SeqCst);
                sh.log(&format!("taskbar: waited {:.0} s, {why}: this one takes it anyway", began.elapsed().as_secs_f64()));
                break;
            }
        }
        WAITING.store(false, Ordering::SeqCst);
        crate::island::apply_visibility(&sh);
    });
}

// Each time Explorer started again while hers held the taskbar: how
// long to give it back for when that makes LOOP_TIMES within
// LOOP_WITHIN (step_away). Kept for her whole run.
pub(super) struct Restarts {
    at: Vec<Instant>,
    away: u32,
}

impl Restarts {
    const fn new() -> Self {
        Restarts { at: Vec::new(), away: 0 }
    }

    fn seen(&mut self, now: Instant) -> Option<Duration> {
        self.at.retain(|&t| now.duration_since(t) < LOOP_WITHIN);
        self.at.push(now);
        if self.at.len() < LOOP_TIMES {
            return None;
        }
        self.at.clear();
        let secs = (AWAY_FIRST << self.away.min(8)).min(AWAY_MOST);
        self.away += 1;
        Some(Duration::from_secs(secs))
    }
}

pub(super) static RESTARTS: Lock<Restarts> = Lock::new("taskbar::win::hold::RESTARTS", Restarts::new());
// Explorer's taskbar's process as last seen while hers is up (0: none yet).
pub(super) static EXPLORER_PID: AtomicU32 = AtomicU32::new(0);

// On the taskbar's thread (each tick, Windows' shown again, TaskbarCreated):
// whether Explorer is another process than the last one seen with a
// taskbar. A new one has ours set behind its tray (come_forward puts it
// first again) and is counted (Restarts).
pub(super) fn explorer_check() -> bool {
    let pid = shell::explorer_pid();
    if pid == 0 || EXPLORER_PID.swap(pid, Ordering::SeqCst) == pid {
        return false;
    }
    let up = explorer_up(pid).map_or_else(|| "gone".to_string(), |d| format!("up {:.1} s", d.as_secs_f64()));
    systray::set_aside(true);
    log(&format!("Explorer started again (pid {pid}, {up}): ours set behind its tray until it has run {} s (zhoupengjie/Wakuwaku#85)", EXPLORER_SETTLE.as_secs()));
    // Read first: an if let's condition holds its lock through the block.
    let away = RESTARTS.lock().unwrap().seen(Instant::now());
    if let Some(away) = away {
        step_away(away);
    }
    true
}

// Set behind a new Explorer's tray: first again once it has run
// EXPLORER_SETTLE, and programs asked for their icons (what they added
// meanwhile went to Explorer's alone).
pub(super) fn come_forward() {
    if !systray::aside() {
        return;
    }
    let pid = EXPLORER_PID.load(Ordering::SeqCst);
    let up = explorer_up(pid);
    if unready(up, true, EXPLORER_SETTLE).is_some() {
        return;
    }
    systray::set_aside(false);
    systray::keep_first();
    log(&format!("Explorer (pid {pid}) up {:.0} s: ours first again", up.unwrap_or_default().as_secs_f64()));
    ASK_AT.store(TICKS.load(Ordering::SeqCst) + SETTLE_TICKS, Ordering::SeqCst);
}

// Explorer started again LOOP_TIMES times within LOOP_WITHIN, hers set
// behind it or not: the taskbar given back (Windows' own shown, her
// window hidden: waiting) until Explorer has run `away`, then taken
// again. The home stays the taskbar: this passes (#69's tug of war, which
// makes it the island, does not). Not on the taskbar's thread: giving it
// back stops that thread and waits for it.
pub(super) fn step_away(away: Duration) {
    log(&format!("Explorer started again {LOOP_TIMES} times within {} s: Windows' taskbar given back until Explorer has run {} s", LOOP_WITHIN.as_secs(), away.as_secs()));
    let Some(sh) = sh() else { return };
    std::thread::spawn(move || {
        let why = format!("Explorer started again {LOOP_TIMES} times within {} s", LOOP_WITHIN.as_secs());
        wait_for_explorer(&sh, &why, away, None);
        crate::island::apply_visibility(&sh);
    });
}

#[cfg(test)]
mod explorer_tests {
    use super::{unready, Restarts, AWAY_FIRST, AWAY_MOST, LOOP_TIMES, LOOP_WITHIN};
    use std::time::{Duration, Instant};

    const SETTLE: Duration = Duration::from_secs(30);

    // #85's two logons: hers before Explorer had a taskbar (and a
    // desktop), and just after; then Explorer settled.
    #[test]
    fn explorer_is_waited_for_until_it_has_settled() {
        assert_eq!(unready(None, false, SETTLE).as_deref(), Some("Explorer has no taskbar yet"));
        assert_eq!(unready(Some(Duration::from_secs(40)), false, SETTLE).as_deref(), Some("Explorer's desktop is not up yet"));
        assert_eq!(unready(Some(Duration::from_millis(2500)), true, SETTLE).as_deref(), Some("Explorer up 2.5 s, under the 30 s it is given to start"));
        assert_eq!(unready(Some(SETTLE), true, SETTLE), None);
        assert_eq!(unready(Some(Duration::from_secs(7200)), true, SETTLE), None);
    }

    // Explorer started again now and then (by hand, an update): never a loop.
    #[test]
    fn a_restart_now_and_then_is_nothing() {
        let (mut r, t) = (Restarts::new(), Instant::now());
        assert!((0..20u64).all(|i| r.seen(t + LOOP_WITHIN * i as u32 / 2).is_none()));
    }

    // #85's: one every two seconds. Given back at the third, for longer
    // each time it loops again, up to the most.
    #[test]
    fn a_crash_loop_gives_it_back_for_longer_each_time() {
        let (mut r, t) = (Restarts::new(), Instant::now());
        let mut away = Vec::new();
        for i in 0..LOOP_TIMES as u64 * 7 {
            if let Some(d) = r.seen(t + Duration::from_secs(2 * i)) {
                away.push((i, d.as_secs()));
            }
        }
        let at: Vec<u64> = away.iter().map(|a| a.0).collect();
        assert_eq!(at, (1..=7).map(|n| n * LOOP_TIMES as u64 - 1).collect::<Vec<_>>());
        let secs: Vec<u64> = away.iter().map(|a| a.1).collect();
        assert_eq!(secs[..4], [AWAY_FIRST, AWAY_FIRST * 2, AWAY_FIRST * 4, AWAY_FIRST * 8]);
        assert!(secs.iter().all(|&s| s <= AWAY_MOST));
        assert_eq!(secs.last(), Some(&AWAY_MOST));
    }

    // The window slides: two, a long while, then two more is no loop.
    #[test]
    fn restarts_far_apart_are_apart() {
        let (mut r, t) = (Restarts::new(), Instant::now());
        assert!(r.seen(t).is_none());
        assert!(r.seen(t + Duration::from_secs(1)).is_none());
        assert!(r.seen(t + LOOP_WITHIN + Duration::from_secs(5)).is_none());
        assert!(r.seen(t + LOOP_WITHIN + Duration::from_secs(6)).is_none());
        assert!(r.seen(t + LOOP_WITHIN + Duration::from_secs(7)).is_some());
    }
}

// Before this one's window shows for the taskbar (island/): should
// another copy hold it, this one waits.
// Nor while Explorer is still starting (explorer_unready): it waits too.
pub fn look_for_other(sh: &Shared) {
    if is_up() || waiting() {
        return;
    }
    if let Some((pid, _)) = other_holder() {
        wait_for_other(sh, pid);
    } else if let Some(why) = explorer_unready(EXPLORER_SETTLE) {
        wait_for_explorer(sh, &why, EXPLORER_SETTLE, Some(EXPLORER_AT_MOST));
    }
}

// Both took it at once (each before the other's window was there): the
// younger lets it go, the elder keeps it; by pid should they have
// started together.
pub(super) fn yield_to_elder() {
    let Some((pid, theirs)) = other_holder() else { return };
    let me = std::process::id();
    if (theirs, pid) >= (shell::started(me), me) {
        return;
    }
    if let Some(sh) = sh() {
        std::thread::spawn(move || {
            wait_for_other(&sh, pid);
            crate::island::apply_visibility(&sh);
        });
    }
}

// Windows' taskbar put away and the tray taken, before the strip is
// laid; false while another copy holds it, or while Explorer is still
// starting (this one then waits).
pub fn take(sh: &Shared, strip: (i32, i32, i32, i32)) -> bool {
    if is_up() {
        return true;
    }
    if let Some((pid, _)) = other_holder() {
        wait_for_other(sh, pid);
        return false;
    }
    if let Some(why) = explorer_unready(EXPLORER_SETTLE) {
        wait_for_explorer(sh, &why, EXPLORER_SETTLE, Some(EXPLORER_AT_MOST));
        return false;
    }
    *SH.lock().unwrap() = Some(crate::shared(&sh.app));
    let file = sh.dir.join("taskbar.json");
    // The shell's windows before hers is up, to compare with after (shell.rs LoneWin).
    sh.log(&format!("taskbar: {}", shell::lone_win().said()));
    // Each step timed, for a start that took long (one took 600 ms
    // where most take 10, 2026-10-13: which step waited on Explorer?).
    let began = Instant::now();
    if let Err(e) = shell::hide(&file) {
        sh.log(&format!("taskbar: could not write {}: {e}", file.display()));
        return false;
    }
    let hidden = began.elapsed();
    let guarded = shell::spawn_guard(&file);
    let guard = began.elapsed() - hidden;
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();
    let thread = crate::spawn_named("taskbar host", move || run(strip, ready_tx));
    match ready_rx.recv_timeout(Duration::from_secs(3)) {
        Ok(hwnd) => *HOST.lock().unwrap() = Some(Host { hwnd, thread }),
        Err(_) => sh.log("taskbar: its thread did not start"),
    }
    let host = began.elapsed() - hidden - guard;
    sh.log(&format!("taskbar: Windows' put away; guard started: {guarded} (put away {} ms, guard {} ms, host {} ms)", hidden.as_millis(), guard.as_millis(), host.as_millis()));
    TASKS_STOP.store(false, Ordering::SeqCst);
    let sh = crate::shared(&sh.app);
    let (keys_sh, input_sh) = (sh.clone(), sh.clone());
    // Its way in first: what the taskbar's thread sees from now on reaches it.
    let notes = notes();
    *TASKS.lock().unwrap() = Some(crate::spawn_named("taskbar buttons", move || tasks_loop(sh, notes)));
    // Which input method is in front, told by Windows' input switcher
    // (inputs.rs): its mark read again at once when it changes.
    crate::inputs::set_size(icon_px(&keys_sh));
    // Its way in made before: an input method switched at once is not missed.
    let asks = asks();
    crate::inputs::start(png_of, Box::new(keys_now_asked), Box::new(move |line: &str| input_sh.log(&format!("taskbar: {line}"))));
    // Win+1…9, Win+T, Win+B hers, not Windows' put away (winkeys.rs).
    crate::winkeys::start(&keys_sh);
    *KEYS.lock().unwrap() = Some(crate::spawn_named("taskbar keys", move || keys_loop(keys_sh, asks)));
    true
}

// A thread asked to stop, waited for two seconds at most: on the way out
// (the main thread, quitting) one that does not stop must not hold her,
// and Windows' taskbar, up; it is left, and goes with her. Each step in
// the log: a quit once hung here, none of it said where (2026-10-11).
pub(super) fn stop_within(name: &str, thread: JoinHandle<()>) -> bool {
    log(&format!("giving back: {name} asked to stop"));
    let asked = Instant::now();
    while !thread.is_finished() && asked.elapsed() < Duration::from_secs(2) {
        std::thread::sleep(Duration::from_millis(10));
    }
    if thread.is_finished() {
        let _ = thread.join();
        return true;
    }
    log(&format!("giving back: {name} did not stop in 2 s: left"));
    false
}

// The tray handed back to Explorer and its taskbar shown again.
pub fn give_back(sh: &Shared) {
    // Windows' own keys first: its taskbar has them again (winkeys.rs).
    crate::winkeys::stop();
    TASKS_STOP.store(true, Ordering::SeqCst);
    // Each thread taken out of its lock first: as an if let's condition
    // the lock was held through the block, over the two seconds' wait for
    // the thread (and the log's and TASKS_DOING's locks inside it).
    let tasks = TASKS.lock().unwrap().take();
    if let Some(thread) = tasks {
        wake_tasks();
        // Nothing more told it: its wait ends (and its own state goes with it).
        no_more_notes();
        if !stop_within("the buttons' thread", thread) {
            let doing = *TASKS_DOING.lock().unwrap();
            log(&format!("giving back: the buttons' thread was {doing}"));
        }
    }
    // Its way in dropped: its wait ends at once (and what it sent last
    // stays, as it did, for a page come up after).
    no_more_asks();
    let keys = KEYS.lock().unwrap().take();
    if let Some(thread) = keys {
        stop_within("the keys' thread", thread);
    }
    if let Some(thread) = crate::inputs::stop() {
        stop_within("the input switcher's thread", thread);
    }
    let host = HOST.lock().unwrap().take();
    let had_host = host.is_some();
    if let Some(host) = host {
        // SAFETY: the thread's own window; a posted message, no pointers.
        unsafe { PostMessageW(host.hwnd as Hwnd, WM_STOP, 0, 0) };
        stop_within("the taskbar's thread", host.thread);
    }
    let file = sh.dir.join("taskbar.json");
    if file.exists() {
        sh.log(&format!("taskbar: Windows' given back: {}", shell::restore(&file)));
    }
    // Explorer's the taskbar again, shown: every window told so
    // (systray/ handed_back), then whose the shell's windows are now.
    if had_host {
        let told = systray::handed_back();
        sh.log(&format!("taskbar: {}; {}", if told { "every window told Explorer's is the taskbar again" } else { "the tray's window still there: no window told" }, shell::lone_win().said()));
    }
    // The desktop's icons as they were (icons.rs), the work area whole
    // again, not waited for: it takes some seconds, the desktop putting
    // them back first. Quitting, she is gone before then, and her guard
    // does it (main.rs). Taken again, the work area is made anew.
    if crate::icons::file(&sh.dir).exists() {
        let (dir, log) = (sh.dir.clone(), crate::shared(&sh.app));
        std::thread::spawn(move || {
            for line in crate::icons::given_back(&dir) {
                log.log(&line);
            }
        });
    }
    *WORK_EDGE.lock().unwrap() = None;
    *WORK_SIDE.lock().unwrap() = None;
    *SENT.lock().unwrap() = Value::Null;
    *SENT_WINDOWS.lock().unwrap() = Value::Null;
    *PAPER.lock().unwrap() = Value::Null;
    *LOOK.lock().unwrap() = Value::Null;
    START_OPEN.store(false, Ordering::SeqCst);
    *STRIP.lock().unwrap() = None;
}

// Windows' taskbar shown again as fast as she puts it away: another
// program shows or hides it too (a dock that keeps it hidden, as
// MyDockFinder: zhoupengjie/Wakuwaku#69), and the two fought over it
// some times a second, the screen stuttering. So many times within any
// so many ticks is that: about a second of #69's, some four seconds of
// the timer alone seeing it shown at every tick, at most ten of one
// showing it once a second or so. In normal use it is next to never
// shown again (none in 71 hours of the taskbar home, 2026-10-20); a few
// at once (Explorer started again on several displays) is nothing.
// Shown again many times within one tick is one time: Explorer's own
// taskbar, put away and set to hide itself (her strip along the top or
// a side), shows itself again in bursts as it takes that state and as a
// window goes full screen, 8 times in 30 to 440 ms, and she left the
// taskbar to nobody (the test VM, 2026-10-30, zhoupengjie/Wakuwaku#98);
// #69's, all the while, is two a second so, caught in some four.
// The displays changed (one turned off, the main one another, a new
// resolution or scale): Explorer lays its taskbars out anew for some
// twenty seconds, setting the work area back and showing its taskbar
// again and again as it goes, a few times a second or a few seconds
// apart, 16 in 12 s as the displays went from both to the second alone
// (2026-11-10, zhoupengjie/Wakuwaku#115): none of it counted until
// TUG_CALM after the last change, the count from nothing then.
// And with no change of displays, a burst of some seconds is no tug of
// war either: on 2026-11-11 at 20:30 its taskbar showed itself in seven
// pairs 0.6 to 0.8 s apart, 5 s in all, for no reason the log tells
// (no full screen, no display, no Explorer started again; a window
// flashing its button did not do it, tried), and 8 of them within 10 s
// made the island her home (#115). A tug of war goes on and on: so many
// within 30 s is that, some twelve seconds of #69's or of the timer
// seeing it shown at every tick, at most 24 of one showing it once a
// second; the screen stutters for those seconds before she gives way.
pub(super) const TUG_TICKS: u32 = 60;
pub(super) const TUG_TIMES: usize = 24;
pub(super) const TUG_CALM: u32 = 60;

pub(super) struct Tug {
    // The ticks of the last TUG_TIMES times (a ring), and how many in all.
    at: [u32; TUG_TIMES],
    seen: usize,
    lost: bool,
    // Not counted before this tick (the displays changed).
    calm_until: u32,
}

impl Tug {
    pub(super) const fn new() -> Self {
        Tug { at: [0; TUG_TIMES], seen: 0, lost: false, calm_until: 0 }
    }

    // The displays changed at this tick: what was counted forgotten,
    // and nothing counted for TUG_CALM.
    fn displays_changed(&mut self, tick: u32) {
        self.seen = 0;
        self.calm_until = self.calm_until.max(tick + TUG_CALM);
    }

    // Shown again once more, at this tick: whether that makes it the
    // tug of war (once; from then on it is left to the other program).
    fn shown(&mut self, tick: u32) -> bool {
        if self.lost || tick < self.calm_until {
            return false;
        }
        // Once already this tick.
        if self.seen > 0 && self.at[(self.seen - 1) % TUG_TIMES] == tick {
            return false;
        }
        self.at[self.seen % TUG_TIMES] = tick;
        self.seen += 1;
        // The oldest of the last TUG_TIMES: the next to be written over.
        self.lost = self.seen >= TUG_TIMES && tick.saturating_sub(self.at[self.seen % TUG_TIMES]) < TUG_TICKS;
        self.lost
    }
}

pub(super) static TUG: Lock<Tug> = Lock::new("taskbar::win::hold::TUG", Tug::new());

// Said once for each time her guard shows Windows' taskbar for a hang.
static LEFT_FOR_HANG: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub(super) fn put_away_again(how: &str) {
    if HANDING_BACK.load(Ordering::SeqCst) || TUG.lock().unwrap().lost {
        return;
    }
    // Shown by her guard while she did not answer: no tug of war, and up
    // till the guard says she answers; then the timer finds it shown.
    if shown_for_hang() {
        if !LEFT_FOR_HANG.swap(true, Ordering::SeqCst) {
            // As the person had it (shell.rs hang_state): one she set to hide
            // itself would stay slid away. This thread answers: its tray
            // hands the message on, as rehide's.
            let state = sh().is_some_and(|sh| shell::hang_state(&sh.dir.join("taskbar.json")));
            log(&format!("Windows' shown by her guard while she did not answer ({how}): left up till she does{}", if state { ", as it was before her" } else { "" }));
        }
        return;
    }
    LEFT_FOR_HANG.store(false, Ordering::SeqCst);
    // A new Explorer's taskbar showing is no tug of war (zhoupengjie/Wakuwaku#85).
    if !explorer_check() && TUG.lock().unwrap().shown(TICKS.load(Ordering::SeqCst)) {
        give_up();
        return;
    }
    shell::rehide();
    // What is in front: the program that showed it, often enough (#115).
    log(&format!("Windows' shown again ({how}), {} in front: put away", crate::fullscreen::front()));
    if systray::keep_first() {
        log("tray: Explorer's window had come first; ours put ahead again");
        ask_soon();
    }
}

// The displays changed (WM_DISPLAYCHANGE, to every top-level window:
// the main display's new size and colour depth with it). Logged, and
// Windows' taskbar showing itself as Explorer lays it out anew is no
// tug of war (zhoupengjie/Wakuwaku#115).
pub(super) fn displays_changed(wparam: usize, lparam: isize) {
    let (w, h) = (lparam & 0xffff, (lparam >> 16) & 0xffff);
    let displays = sh().map(|sh| sh.displays()).unwrap_or_default();
    log(&format!("the displays changed: the main one {w}x{h}, {wparam} bits; now {displays:?}; Windows' taskbar showing again not counted for {} s", TUG_CALM / 2));
    TUG.lock().unwrap().displays_changed(TICKS.load(Ordering::SeqCst));
}

// The tug of war given up: Windows' taskbar left to the other program,
// her home the island (kept, so the next start does not fight again),
// and the island saying why, its button opening 外观 to go back.
pub(super) fn give_up() {
    log(&format!("Windows' shown again {TUG_TIMES} times within {} s: another program shows or hides it too; home made the island", TUG_TICKS / 2));
    let Some(sh) = sh() else { return };
    // Not on this thread: going home stops it and waits for it (give_back).
    std::thread::spawn(move || {
        crate::shared::set_display(&sh, "island");
        // The island settled in its new home first.
        std::thread::sleep(Duration::from_secs(2));
        let lang = sh.lang();
        let (words, label) = (crate::i18n::t(lang, "taskbar.tug"), crate::i18n::t(lang, "taskbar.tugTap"));
        sh.to_pages("pet:nudge", json!({ "id": "taskbar-tug", "icon": "flag", "words": words, "label": label, "value": "", "private": false, "open": { "tab": "look" } }));
    });
}

#[cfg(test)]
mod tug_tests {
    use super::{Tug, TUG_CALM, TUG_TICKS, TUG_TIMES};

    // Shown again now and then (once every three ticks, for a long
    // while: TUG_TIMES of them never within TUG_TICKS): never a tug of war.
    #[test]
    fn now_and_then_is_nothing() {
        let mut tug = Tug::new();
        assert!((0..2000).step_by(3).all(|tick| !tug.shown(tick)));
    }

    // A burst within a tick or two (Explorer's own taskbar showing itself
    // as it is set to hide itself, or as a window goes full screen): no
    // tug of war (zhoupengjie/Wakuwaku#98).
    #[test]
    fn a_burst_is_nothing() {
        let mut tug = Tug::new();
        assert!((0..TUG_TIMES * 2).all(|_| !tug.shown(100)));
        assert!((0..TUG_TIMES * 2).all(|i| !tug.shown(200 + (i / TUG_TIMES) as u32)));
    }

    // Shown again 7 or 8 times a second all the while (its show events,
    // #69's): a tug of war, told once, in some twelve seconds.
    #[test]
    fn shown_again_all_the_while_is_a_tug_of_war() {
        let mut tug = Tug::new();
        // Every 130 ms, the tick 500 ms.
        let told: Vec<u32> = (0..100u32).filter(|i| tug.shown(i * 130 / 500)).map(|i| i * 130).collect();
        assert_eq!(told.len(), 1);
        assert!((11000..=12500).contains(&told[0]), "{told:?}");
    }

    // Seen by the timer at every tick, put away and shown again each
    // time: the TUG_TIMES-th tick (twelve seconds).
    #[test]
    fn every_tick_is_a_tug_of_war() {
        let mut tug = Tug::new();
        let at = (0..TUG_TICKS * 2).find(|&tick| tug.shown(50 + tick));
        assert_eq!(at, Some(TUG_TIMES as u32 - 1));
    }

    // Shown again once a second (every other tick), on and on: caught
    // within TUG_TICKS.
    #[test]
    fn once_a_second_is_a_tug_of_war() {
        let mut tug = Tug::new();
        let at = (0..200).step_by(2).find(|&tick| tug.shown(tick));
        assert_eq!(at, Some(2 * (TUG_TIMES as u32 - 1)));
    }

    // Any TUG_TICKS, not from the first time: a lone one, then half of
    // them just before the first one's TUG_TICKS are up and half just
    // after (counted from the first, both halves would fall short).
    #[test]
    fn the_window_slides() {
        let half = TUG_TIMES as u32 / 2;
        let mut tug = Tug::new();
        assert!(!tug.shown(0));
        assert!((0..half).all(|i| !tug.shown(TUG_TICKS - half + i)));
        let told: Vec<bool> = (0..half).map(|i| tug.shown(TUG_TICKS + 1 + i)).collect();
        assert_eq!(told.last(), Some(&true));
    }

    // A burst of some seconds with nothing else to tell why, as on
    // 2026-11-11 at 20:30 (#115: seven pairs in 5 s, then the timer, by
    // the tick from the log), and another a minute later: no tug of war.
    #[test]
    fn a_few_seconds_of_it_is_nothing() {
        let mut tug = Tug::new();
        let burst = [33, 33, 34, 34, 36, 36, 37, 37, 38, 39, 39, 40, 40, 42];
        assert!(burst.iter().all(|&tick| !tug.shown(tick)));
        assert!(burst.iter().all(|&tick| !tug.shown(tick + 120)));
    }

    // The displays changed: Explorer's taskbar shown again as it lays
    // itself out anew, as on 2026-11-10 (#115: 16 times in some 12 s,
    // by the tick from the log), is no tug of war; counted again from
    // nothing after TUG_CALM, and a real one caught then.
    #[test]
    fn the_displays_changing_is_nothing() {
        let mut tug = Tug::new();
        // Some shown before, from a full screen.
        assert!(!tug.shown(0) && !tug.shown(2));
        tug.displays_changed(5);
        let after = [20, 23, 23, 31, 32, 33, 34, 34, 35, 35, 45];
        assert!(after.iter().all(|&tick| !tug.shown(tick)));
        // Then just before the calm is over, and after: from nothing.
        assert!(!tug.shown(5 + TUG_CALM - 1));
        let at = (0..TUG_TICKS * 2).find(|&i| tug.shown(5 + TUG_CALM + i));
        assert_eq!(at, Some(TUG_TIMES as u32 - 1));
    }

    // Changed again while calm: the calm from the last change.
    #[test]
    fn the_calm_from_the_last_change() {
        let mut tug = Tug::new();
        tug.displays_changed(0);
        tug.displays_changed(30);
        assert!((0..TUG_TIMES as u32).all(|i| !tug.shown(30 + TUG_CALM - TUG_TIMES as u32 + i)));
        // A change before an earlier one's calm is up never shortens it.
        tug.displays_changed(1);
        assert!(!tug.shown(30 + TUG_CALM - 1));
    }

    // A run of ticks, then quiet, then another: each counted alone.
    #[test]
    fn runs_far_apart_are_apart() {
        let n = TUG_TIMES as u32 - 1;
        let mut tug = Tug::new();
        assert!((0..n).all(|i| !tug.shown(10 + i)));
        assert!((0..n).all(|i| !tug.shown(10 + n + TUG_TICKS + i)));
    }
}

// Windows' taskbar on a display as it is on screen now, across the
// display (physical px), as a PNG data URL: laid over it by her page
// before it is put away (island/ handoff_cover). None while it does
// not show there (auto-hidden, put away), or should the screen not read.
pub fn strip_picture(mon: (i32, i32, i32, i32)) -> Option<String> {
    let (x, y, w, h) = shell::shown_room(mon)?;
    if w <= 0 || h <= 0 || w > 16_384 || h > 512 {
        return None;
    }
    let n = (w * h) as usize;
    // SAFETY: the screen's DC released, our own DC, bitmap and pixels
    // freed; the pixels read from the bitmap while selected.
    let bgra = unsafe {
        let screen = GetDC(std::ptr::null_mut());
        if screen == 0 {
            return None;
        }
        let dc = CreateCompatibleDC(screen);
        let info = BitmapInfoHeader { size: std::mem::size_of::<BitmapInfoHeader>() as u32, width: w, height: -h, planes: 1, bit_count: 32, compression: 0, size_image: 0, x_ppm: 0, y_ppm: 0, clr_used: 0, clr_important: 0 };
        let mut bits: *mut u8 = std::ptr::null_mut();
        let bmp = CreateDIBSection(dc, &info, 0, &mut bits, 0, 0);
        let pixels = if bmp == 0 || bits.is_null() {
            None
        } else {
            let old = SelectObject(dc, bmp);
            // SRCCOPY | CAPTUREBLT: what is composed there, layered windows too.
            let ok = BitBlt(dc, 0, 0, w, h, screen, x, y, 0x00CC_0020 | 0x4000_0000) != 0;
            let pixels = ok.then(|| std::slice::from_raw_parts(bits, n * 4).to_vec());
            SelectObject(dc, old);
            DeleteObject(bmp);
            pixels
        };
        DeleteDC(dc);
        ReleaseDC(std::ptr::null_mut(), screen);
        pixels
    }?;
    // BGR to RGB, all of it opaque.
    let mut rgba = bgra;
    for px in rgba.chunks_exact_mut(4) {
        px.swap(0, 2);
        px[3] = 255;
    }
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, w as u32, h as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().ok()?;
        writer.write_image_data(&rgba).ok()?;
    }
    Some(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(out)))
}
