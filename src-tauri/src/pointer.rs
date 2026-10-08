// Clicks through a transparent window, except on what it draws, for both
// windows (pet.rs, island.rs).
//
// A window that lets clicks through hears no mouse moves (Electron could
// forward them; Tauri cannot). So while the pointer is near, look() hands
// back where it is in the page (CSS pixels), for src/pet/bridge.js to play
// the mouseenter / mouseleave the page waits for. The page then says when the
// pointer is over something of its own (set_over), and the window takes the click.
use serde_json::{json, Value};

// The pointer this close to the window (logical px) counts as near.
const NEAR_PX: f64 = 24.0;

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
        Some(mode != Mode::Catch)
    }

    // One look at the cursor (physical px) against the window's rect. held:
    // the window keeps the pointer (a drag); passive: it takes no mouse at all.
    pub fn look(&mut self, cursor: (i32, i32), pos: (i32, i32), size: (i32, i32), sf: f64, held: bool, passive: bool) -> Look {
        let moved = cursor != self.last;
        self.last = cursor;
        if passive {
            let ignore = self.force(Mode::Pass);
            return Look { ignore, pointer: self.gone(), near: false, moved, forward: false };
        }
        let near = within(cursor, pos, size, (NEAR_PX * sf) as i32);
        // Over something of ours, yet outside the window: the page missed the
        // pointer leaving (it was held elsewhere). Stop taking clicks there.
        if self.over && !held && !within(cursor, pos, size, 0) {
            self.over = false;
        }
        let mode = if held || self.over { Mode::Catch } else if near { Mode::Forward } else { Mode::Pass };
        let ignore = self.force(mode);
        let pointer = if mode == Mode::Forward && moved {
            self.pointing = true;
            Some(json!({ "x": (cursor.0 - pos.0) as f64 / sf, "y": (cursor.1 - pos.1) as f64 / sf }))
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

    // The page says the pointer is on something of its own, or not.
    pub fn set_over(&mut self, over: bool, held: bool, passive: bool) -> Option<bool> {
        self.over = over;
        if passive || held {
            return None;
        }
        self.force(if over { Mode::Catch } else { Mode::Forward })
    }

    // Forget the pointer was over anything (the window moved from under it).
    pub fn reset(&mut self, passive: bool) -> Option<bool> {
        self.over = false;
        self.mode = Mode::Unset;
        self.force(if passive { Mode::Pass } else { Mode::Forward })
    }
}
