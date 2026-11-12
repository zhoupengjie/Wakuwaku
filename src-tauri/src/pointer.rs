// Clicks through a transparent window, except on what it draws, for both
// windows (pet.rs, island/).
//
// A window that lets clicks through hears no mouse moves (Electron could
// forward them; Tauri cannot). So while the pointer is near, look() hands
// back where it is in the page (CSS pixels), for src/pet/bridge.ts to play
// the mouseenter / mouseleave the page waits for. The page then says when the
// pointer is over something of its own (set_over), and the window takes the click.
//
// Caught, the page has the mouse itself, yet its mouseleave may never come:
// entered by a played mouseenter, WebView2 has seen no move of its own over
// her, so its first move is already off her and it has nothing to leave
// (the test VM's taskbar island stayed open for good after a hover, its
// window taking every click over the bottom of the screen, 2026-10-16). So
// caught, where the pointer is goes to the page too as it moves (caught:
// true), and bridge.ts leaves what the pointer is no longer on. Only while
// the page says the pointer is on her, and only as it moves: nothing at rest.
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

// The pointer this close to the window (logical px) counts as near.
const NEAR_PX: f64 = 24.0;
// The mode changing this many times within this long is bouncing (the page
// says the pointer is on her, the next look says it is not, and so on 30
// times a second, 2026-10-11): then the window stays caught for CALM_FOR.
// Each change sets the window's style and logs a line.
const BOUNCES: usize = 4;
const BOUNCE_WITHIN: Duration = Duration::from_millis(400);
const CALM_FOR: Duration = Duration::from_millis(500);

#[derive(Clone, Copy, PartialEq, Default, Debug)]
pub enum Mode {
    #[default]
    Unset,
    // Clicks pass through.
    Pass,
    // Clicks pass through; the page hears where the pointer is.
    Forward,
    // The window takes the mouse.
    Catch,
}

#[derive(Default)]
pub struct ClickThrough {
    mode: Mode,
    pub over: bool,
    pointing: bool,
    last: (i32, i32),
    // When the mode changed, lately; until when it is kept caught (bouncing).
    changes: VecDeque<Instant>,
    calm_until: Option<Instant>,
    // A bounce just calmed, for one line in the log.
    calmed: bool,
    // The pointer, at the last look, on one of the solid rects (look): the
    // window keeps the mouse there whatever the page says.
    on_solid: bool,
}

pub struct Look {
    // Some(ignore) when the window's mouse mode has to change.
    pub ignore: Option<bool>,
    // For the page: where the pointer is, or Null once it has gone.
    pub pointer: Option<Value>,
    pub near: bool,
    pub moved: bool,
    // Telling the page where the pointer is: look again soon.
    pub forward: bool,
}

pub fn within((cx, cy): (i32, i32), (x, y): (i32, i32), (w, h): (i32, i32), margin: i32) -> bool {
    cx >= x - margin && cx <= x + w + margin && cy >= y - margin && cy <= y + h + margin
}

impl ClickThrough {
    // Some(ignore) when the mode changed.
    pub fn force(&mut self, mode: Mode) -> Option<bool> {
        if self.mode == mode {
            return None;
        }
        self.mode = mode;
        let now = Instant::now();
        self.changes.push_back(now);
        while self.changes.front().is_some_and(|t| now.duration_since(*t) > BOUNCE_WITHIN) {
            self.changes.pop_front();
        }
        if mode == Mode::Catch && self.changes.len() >= BOUNCES && !self.calm() {
            self.calm_until = Some(now + CALM_FOR);
            self.calmed = true;
            self.changes.clear();
        }
        Some(mode != Mode::Catch)
    }

    // Kept caught for a while after bouncing.
    fn calm(&self) -> bool {
        self.calm_until.is_some_and(|t| Instant::now() < t)
    }

    // True once, as a bounce is calmed: for the log.
    pub fn calmed(&mut self) -> bool {
        std::mem::take(&mut self.calmed)
    }

    // One look at the cursor (physical px) against the window's rect. held:
    // the window keeps the pointer (a drag); passive: it takes no mouse at
    // all; solid: rects in the page (CSS px: x, y, w, h) where the window
    // takes the mouse whatever the page says (the taskbar's layout being
    // edited: its bar of buttons, the library, the tools; island.ts), so a
    // press there is never let through to the window under it, nor its
    // mouse mode flip back and forth while the pointer is on them
    // (zhoupengjie/Wakuwaku#53: 完成 and 取消 did nothing, the presses going
    // under it and only their release reaching the page).
    pub fn look(&mut self, cursor: (i32, i32), pos: (i32, i32), size: (i32, i32), sf: f64, page_sf: f64, held: bool, passive: bool, solid: &[[f64; 4]]) -> Look {
        let moved = cursor != self.last;
        self.last = cursor;
        if passive {
            self.on_solid = false;
            let ignore = self.force(Mode::Pass);
            return Look { ignore, pointer: self.gone(), near: false, moved, forward: false };
        }
        let (px, py) = ((cursor.0 - pos.0) as f64 / page_sf, (cursor.1 - pos.1) as f64 / page_sf);
        self.on_solid = solid.iter().any(|r| px >= r[0] && px <= r[0] + r[2] && py >= r[1] && py <= r[1] + r[3]);
        let near = within(cursor, pos, size, (NEAR_PX * sf) as i32);
        // Over something of ours, yet outside the window: the page missed the
        // pointer leaving (it was held elsewhere). Stop taking clicks there,
        // and tell the page it has gone (gone(), or where it is now), so
        // what it still has entered is left. Not while calming a bounce: it
        // stays caught.
        if self.over && !held && !within(cursor, pos, size, 0) && !self.calm() {
            self.over = false;
            self.pointing = true;
        }
        // Kept caught while a bounce is calmed, or on a solid rect, whatever
        // the page said meanwhile.
        let mode = if held || self.over || self.on_solid || self.calm() { Mode::Catch } else if near { Mode::Forward } else { Mode::Pass };
        let ignore = self.force(mode);
        let at = || json!({ "x": (cursor.0 - pos.0) as f64 / page_sf, "y": (cursor.1 - pos.1) as f64 / page_sf });
        let pointer = if mode == Mode::Forward && moved {
            self.pointing = true;
            Some(at())
        } else if mode == Mode::Catch && moved && self.over && !held {
            // Caught, as it moves: for the page to check what it has entered.
            let mut at = at();
            at["caught"] = json!(true);
            Some(at)
        } else if mode == Mode::Pass {
            self.gone()
        } else {
            None
        };
        Look { ignore, pointer, near, moved, forward: mode == Mode::Forward }
    }

    fn gone(&mut self) -> Option<Value> {
        std::mem::take(&mut self.pointing).then_some(Value::Null)
    }

    // The page says the pointer is on something of its own, or not. Said
    // not while a bounce is calmed, it is kept, and the window stays caught
    // only until the calm is over (look). It was dropped: the window stayed
    // caught over the lower half of the screen, taking every click, until
    // the pointer went above it or over her again (2026-10-12).
    // Said not with the pointer on a solid rect: the window stays caught.
    pub fn set_over(&mut self, over: bool, held: bool, passive: bool) -> Option<bool> {
        self.over = over;
        if passive || held || (!over && (self.calm() || self.on_solid)) {
            return None;
        }
        self.force(if over { Mode::Catch } else { Mode::Forward })
    }

    // Whether the window takes the mouse now (for /health).
    pub fn caught(&self) -> bool {
        self.mode == Mode::Catch
    }

    // Forget the pointer was over anything (the window moved from under it).
    pub fn reset(&mut self, passive: bool) -> Option<bool> {
        self.over = false;
        self.mode = Mode::Unset;
        self.force(if passive { Mode::Pass } else { Mode::Forward })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The page says on her, the next look says not, over and over: after a
    // few of those the window stays caught instead of changing its style
    // thirty times a second.
    #[test]
    fn a_bounce_is_calmed() {
        let mut ct = ClickThrough::default();
        let (pos, size) = ((0, 0), (100, 100));
        let inside = (50, 50);
        let mut changes = 0;
        for _ in 0..2 {
            changes += ct.set_over(true, false, false).is_some() as usize;
            changes += ct.set_over(false, false, false).is_some() as usize;
        }
        assert_eq!(changes, 4);
        assert!(!ct.calmed(), "four changes are not yet a bounce");
        assert!(ct.set_over(true, false, false).is_some());
        assert!(ct.calmed(), "the third catch within the moment calms it");
        assert!(!ct.calmed(), "said once");
        // Calmed: the page saying not on her is kept, yet the window stays
        // caught, inside it or out.
        assert_eq!(ct.set_over(false, false, false), None);
        assert!(!ct.over);
        let seen = ct.look((500, 500), pos, size, 1.0, 1.0, false, false, &[]);
        assert_eq!(seen.ignore, None);
        let seen = ct.look(inside, pos, size, 1.0, 1.0, false, false, &[]);
        assert_eq!(seen.ignore, None);
        // The calm over: the next look lets clicks through again (the
        // pointer still in the window, as over the lower half of the screen).
        ct.calm_until = Some(Instant::now() - Duration::from_millis(1));
        let seen = ct.look(inside, pos, size, 1.0, 1.0, false, false, &[]);
        assert_eq!(seen.ignore, Some(true));
        assert!(seen.forward);
    }

    // Calmed while the page still says on her: caught on, and once the calm
    // is over still caught for as long as she is under the pointer.
    #[test]
    fn calmed_over_her_stays_caught() {
        let mut ct = ClickThrough::default();
        let (pos, size) = ((0, 0), (100, 100));
        for _ in 0..2 {
            ct.set_over(true, false, false);
            ct.set_over(false, false, false);
        }
        ct.set_over(true, false, false);
        assert!(ct.calmed());
        ct.calm_until = Some(Instant::now() - Duration::from_millis(1));
        let seen = ct.look((50, 50), pos, size, 1.0, 1.0, false, false, &[]);
        assert_eq!(seen.ignore, None);
        assert!(ct.over);
    }

    // Caught over her, the page hears where the pointer goes as it moves
    // (marked caught), to leave what it is no longer on; not at rest, not
    // while held (a drag), not once the page says it is off her.
    #[test]
    fn caught_the_page_hears_the_pointer_as_it_moves() {
        let mut ct = ClickThrough::default();
        let (pos, size) = ((100, 200), (400, 300));
        assert_eq!(ct.set_over(true, false, false), Some(false));
        let seen = ct.look((300, 250), pos, size, 1.25, 1.25, false, false, &[]);
        assert_eq!(seen.ignore, None);
        assert_eq!(seen.pointer, Some(json!({ "x": 160.0, "y": 40.0, "caught": true })));
        assert!(!seen.forward);
        // At rest: nothing.
        assert_eq!(ct.look((300, 250), pos, size, 1.25, 1.25, false, false, &[]).pointer, None);
        // Held (a drag): nothing.
        assert_eq!(ct.look((310, 250), pos, size, 1.25, 1.25, true, false, &[]).pointer, None);
        // Off her (the page said so): forwarded as before, not caught.
        assert_eq!(ct.set_over(false, false, false), Some(true));
        let seen = ct.look((320, 250), pos, size, 1.25, 1.25, false, false, &[]);
        assert_eq!(seen.pointer, Some(json!({ "x": 176.0, "y": 40.0 })));
    }

    // The page said on her and never heard it leave; the pointer is out of
    // the window now: clicks pass through again, and the page is told it
    // has gone, even though nothing was forwarded to it before.
    #[test]
    fn a_leave_the_page_missed_is_told() {
        let mut ct = ClickThrough::default();
        let (pos, size) = ((0, 0), (100, 100));
        ct.set_over(true, false, false);
        let _ = ct.look((50, 50), pos, size, 1.0, 1.0, false, false, &[]);
        let seen = ct.look((500, 500), pos, size, 1.0, 1.0, false, false, &[]);
        assert!(!ct.over);
        assert_eq!(seen.ignore, Some(true));
        assert_eq!(seen.pointer, Some(Value::Null));
        // Once.
        assert_eq!(ct.look((510, 500), pos, size, 1.0, 1.0, false, false, &[]).pointer, None);
    }

    // A solid rect (the taskbar's layout being edited, #53): caught while
    // the pointer is on it, whatever the page says; through again off it.
    #[test]
    fn a_solid_rect_keeps_the_mouse() {
        let mut ct = ClickThrough::default();
        let (pos, size) = ((0, 500), (1000, 400));
        let bar = [[100.0, 100.0, 300.0, 40.0]];
        // On it (page 200, 120 at 125 %): caught, the page having said nothing.
        let seen = ct.look((250, 650), pos, size, 1.25, 1.25, false, false, &bar);
        assert_eq!(seen.ignore, Some(false));
        assert!(ct.caught());
        // The page says off (a leave of WebView2's own, the pointer on it): kept.
        assert_eq!(ct.set_over(false, false, false), None);
        assert!(ct.caught());
        assert_eq!(ct.look((260, 650), pos, size, 1.25, 1.25, false, false, &bar).ignore, None);
        // Off it, still in the window: clicks through again, forwarded.
        let seen = ct.look((600, 650), pos, size, 1.25, 1.25, false, false, &bar);
        assert_eq!(seen.ignore, Some(true));
        assert!(seen.forward);
        // Editing over (no rects): as before.
        assert_eq!(ct.look((250, 650), pos, size, 1.25, 1.25, false, false, &[]).ignore, None);
        assert!(!ct.caught());
    }

    #[test]
    fn solid_rect_uses_webview_scale_when_it_differs_from_display_scale() {
        let mut ct = ClickThrough::default();
        let pos = (0, 0);
        let size = (1000, 800);
        let settings = [[40.0, 500.0, 520.0, 100.0]];
        // Windows reports 100% for the monitor, while WebView2 reports
        // 125%: a physical y of 650 is page y 520, inside the settings.
        let seen = ct.look((100, 650), pos, size, 1.0, 1.25, false, false, &settings);
        assert_eq!(seen.ignore, Some(false));
        assert!(ct.caught());
    }

    #[test]
    fn two_changes_are_no_bounce() {
        let mut ct = ClickThrough::default();
        assert_eq!(ct.set_over(true, false, false), Some(false));
        assert_eq!(ct.set_over(false, false, false), Some(true));
        assert!(!ct.calmed());
        assert!(!ct.over);
    }
}
