// What the strip's right end shows of Windows: the keyboard (layout, input
// method, Caps Lock), the volume, the network, the battery, and what is
// using the microphone, camera or location; looked at on a thread of its own
// (keys_loop) and sent to the page as it changes (taskbar:keys).
//
// What it looked at last is that thread's alone; others ask it to look
// (ask_keys: an Ask over a channel) and read what it sent last as it
// published it (SENT_KEYS, never written by another).
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};

use super::*;

// What the keys' thread is asked.
pub(super) enum Ask {
    // The IME's mark and the volume read at the next look, not the next
    // second: the input method was just switched (our button, Windows'
    // input switcher's word).
    Now,
    // Looked at now: the quiet over, or asked to stop.
    Wake,
}

// The way to the keys' thread while it runs (asks: set by hold.rs take
// before it starts it, dropped by give_back, which ends its wait at
// once). Only ever cloned out.
static TO_KEYS: Lock<Option<Sender<Ask>>> = Lock::ranked("taskbar::win::keys::TO_KEYS", crate::lock::LEAF, None);
// What the keys' thread sent last, as it published it (keys_now, a new
// page's resend): swapped whole, read without a copy under the lock.
pub(super) static SENT_KEYS: Lock<Option<Arc<Value>>> = Lock::ranked("taskbar::win::keys::SENT_KEYS", crate::lock::LEAF, None);

pub(super) fn asks() -> Receiver<Ask> {
    let (to, from) = std::sync::mpsc::channel();
    *TO_KEYS.lock().unwrap() = Some(to);
    from
}

pub(super) fn no_more_asks() {
    *TO_KEYS.lock().unwrap() = None;
}

pub(super) fn ask_keys(ask: Ask) {
    let to = TO_KEYS.lock().unwrap().clone();
    if let Some(to) = to {
        let _ = to.send(ask);
    }
}

// A small icon's size on her display (physical px), as Windows' taskbar draws its tray's.
pub(super) fn icon_px(sh: &Shared) -> i32 {
    (16.0 * crate::island::scale(sh)).round() as i32
}

// The keyboard (tasks::keys) and the volume (audio.rs) looked at four
// times a second; the network's way out (and the Wi-Fi's bars) and the
// battery (the quick settings' button) and what is using the microphone, camera or
// location (tasks::in_use) every two; sent when any of it changes.
// The language and Caps Lock (answered in this process) four times a
// second; the IME's mode (asked of its window) and the volume (asked of
// Windows' audio service) once a second, and at once when another
// window comes to the front; the network's way out and what is using
// the microphone, camera or location every two seconds. All of it was
// asked four times a second (2026-10-11). Which input method it is
// (inputs.rs) as last told by Windows' input switcher, read there only
// when it or the layout changes; its icons sent once each
// (taskbar:imeIcons, { key: png }), named by key in the keys' ime.
pub(super) fn keys_loop(sh: Arc<Shared>, asks: Receiver<Ask>) {
    crate::power::thread_eco();
    let _com = crate::audio::Com::new();
    let mut speakers = crate::audio::Reader::new();
    let mut sent = Value::Null;
    let using = || Value::Object(tasks::in_use().into_iter().map(|(what, names)| (what.to_string(), json!(names))).collect());
    // The Wi-Fi's bars only while it is the way out (tasks::wifi_bars).
    let bars = |net: &str| if net == "wifi" { tasks::wifi_bars() } else { None };
    let (mut net, mut used, mut battery) = (tasks::net(), using(), crate::widgets::battery_state());
    let mut wifi = bars(net);
    let (mut native, mut volume, mut front, mut shift_was) = (None, None, 0isize, false);
    let (mut looks, mut now_asked) = (0u32, false);
    while !TASKS_STOP.load(Ordering::SeqCst) {
        looks += 1;
        if looks % 8 == 0 {
            net = tasks::net();
            wifi = bars(net);
            used = using();
            battery = crate::widgets::battery_state();
            crate::inputs::set_size(icon_px(&sh));
        }
        let keys = tasks::keys_cheap();
        crate::inputs::layout(keys.hkl);
        let now_front = tasks::front();
        // Shift let go (it switches many IMEs between scripts), or our
        // own IME button pressed: the mark read now.
        let shift = tasks::shift_down();
        let asked = std::mem::take(&mut now_asked) || (shift_was && !shift);
        shift_was = shift;
        if looks % 4 == 1 || now_front != front || asked {
            front = now_front;
            native = tasks::ime_native();
            volume = speakers.read().map(|(level, muted)| json!([level, muted]));
        }
        send_ime_icons(&sh, false);
        let now = json!({ "lang": keys.lang, "native": native, "caps": keys.caps, "ime": crate::inputs::now(), "net": net, "bars": wifi, "use": used, "volume": volume, "battery": battery });
        if now != sent {
            let _ = sh.app.emit_to("island", "taskbar:keys", now.clone());
            // Split, quick settings perhaps in the dock (bars.rs).
            crate::bars::keys(&sh, &now);
            *SENT_KEYS.lock().unwrap() = Some(Arc::new(now.clone()));
            sent = now;
        }
        // Quiet (nobody at the keyboard): every two seconds; woken at
        // once when asked (the IME button pressed, the quiet over), or
        // when its way in is dropped (give_back). It looked every
        // twentieth whether it was: 20 wakes a second (#14). All asked
        // meanwhile taken at once: one look answers them.
        let nap = Duration::from_millis(if sh.is_quiet() { 2000 } else { 250 });
        match asks.recv_timeout(nap) {
            Ok(ask) => {
                now_asked |= matches!(ask, Ask::Now);
                while let Ok(ask) = asks.try_recv() {
                    now_asked |= matches!(ask, Ask::Now);
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            // Given back (or taken again, a new way in made): done.
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
}

pub(super) fn wake_keys() {
    ask_keys(Ask::Wake);
}

// The IME's mark read at once (its button pressed, the input method switched).
pub(super) fn keys_now_asked() {
    ask_keys(Ask::Now);
}

// The input methods' icons the page has not had (all of them for a page
// just come up), before the keys that name them.
pub(super) fn send_ime_icons(sh: &Shared, all: bool) {
    let icons = crate::inputs::icons(all);
    if !icons.is_empty() {
        let _ = sh.app.emit_to("island", "taskbar:imeIcons", Value::Object(icons.into_iter().map(|(k, png)| (k, json!(png))).collect()));
    }
}

// The keyboard, the network, the volume and the battery as last sent (keys_loop).
pub fn keys_now() -> Value {
    let sent = SENT_KEYS.lock().unwrap().clone();
    sent.map_or(Value::Null, |keys| (*keys).clone())
}
