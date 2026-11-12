// Her settings as the Rust side reads them (config.json in her data folder,
// data.rs load and save): each key declared here once, with its kind and
// its default. A key is one of these constants, never a string typed where
// it is read (Shared::setting, Shared::flag), so one misspelt is a compile
// error, not a setting quietly never found; the kinds tell a switch (Flag:
// off unless it is true) from a value read as it is (Setting: Null while
// unset). The file itself is as it was: a JSON object of every key, and a
// key the pages write that is not declared here (theirs alone) is kept and
// written back with the rest.
use serde_json::{json, Map, Value};

// A switch: on only when it is true (absent, null or anything else, off).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Flag(&'static str);

// A setting read as it is: a string, a number, a list, an object; Null
// while unset. Each reader says what it makes of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Setting(&'static str);

// Either kind, by its key in the file.
pub trait Key: Copy {
    fn name(self) -> &'static str;
}

impl Flag {
    pub const fn name(self) -> &'static str {
        self.0
    }
}

impl Setting {
    pub const fn name(self) -> &'static str {
        self.0
    }
}

impl Key for Flag {
    fn name(self) -> &'static str {
        self.0
    }
}

impl Key for Setting {
    fn name(self) -> &'static str {
        self.0
    }
}

// A switch's value as Shared::flag reads it.
pub fn is_on(value: &Value) -> bool {
    value.as_bool().unwrap_or(false)
}

// The keys: `NAME: Kind = "key" => default` (read here, with a default),
// `NAME: Kind = "key"` (read here, none: unset till the pages or she set it),
// `"key" => default` (a default for the pages, never read here).
macro_rules! keys {
    ($( $($name:ident: $kind:ident =)? $key:literal $(=> $default:tt)?, )*) => {
        $( $( pub const $name: $kind = $kind($key); )? )*

        // Every setting with a default, as a new config starts (data.rs load).
        pub fn defaults() -> Map<String, Value> {
            let mut map = Map::new();
            $( $( map.insert($key.to_string(), json!($default)); )? )*
            map
        }

        // Every key declared, with a default or not.
        #[cfg(test)]
        pub const NAMES: &[&str] = &[$($key),*];
    };
}

keys! {
    LANG: Setting = "lang" => "auto",
    PET: Setting = "pet" => "claude-chan",
    SCALE: Setting = "scale" => 0.55,
    // Her home: island or taskbar (the taskbar from the first start, on
    // Windows; elsewhere there is no taskbar of hers, only the island);
    // out, she is on the desktop.
    DISPLAY: Setting = "display" => (if cfg!(windows) { "taskbar" } else { "island" }),
    OUT: Flag = "out" => false,
    // The compact island's width: narrow, normal or wide (island.ts WIDTHS).
    "islandWidth" => "normal",
    // Where it sits at the top of the screen: left, center or right
    // (anchor.ts, zhoupengjie/Wakuwaku#59); the top centre as ever.
    ISLAND_POS: Setting = "islandPos" => "center",
    // Where it is: top (the top of the screen, as ever), or taskbar, in
    // Windows' own taskbar along whichever edge that is, which stays as
    // it is (island.rs fit, zhoupengjie/Wakuwaku#105).
    ISLAND_EDGE: Setting = "islandEdge" => "top",
    // In the taskbar, her capsule inside it, taking one end of it
    // (embed.rs), unless turned off: laid over it then.
    ISLAND_EMBED: Setting = "islandEmbed" => true,
    // At the top of the screen, slid up out of the way while a window
    // lies under it (island-plate.ts setStow, zhoupengjie/Wakuwaku#49).
    ISLAND_HIDE_UNDER: Flag = "islandHideUnder" => true,
    // The taskbar's strip: mica (the desktop's picture through it, mica.ts),
    // black (solid: black, or light in Windows' light mode) or clear (the
    // desktop itself through it, a new user's since 2026-10-13); clear,
    // Mica or solid while a window is maximized or full screen on its
    // display (mica, solid), or clear always.
    TASKBAR_MATERIAL: Setting = "taskbarMaterial" => "clear",
    // The display her home is on (zhoupengjie/Wakuwaku#104): a display's
    // key (screen.rs), or "" for Windows' main display, as ever.
    HOME_DISPLAY: Setting = "homeDisplay" => "",
    // Her taskbar on her display alone (no taskbar on the others)
    // or on all of them (bars.rs).
    TASKBAR_DISPLAYS: Setting = "taskbarDisplays" => "main",
    TASKBAR_CLEAR_WHEN: Setting = "taskbarClearWhen" => "mica",
    // Its programs' buttons: icons alone or with their titles; in the
    // middle of the screen or after her end (island-layout.ts alignApps).
    "taskbarButtons" => "icons",
    TASKBAR_ALIGN: Setting = "taskbarAlign" => "center",
    // The screen's edge it is along: bottom (Windows' own place), top,
    // left or right (side bars: an upright capsule, icons alone).
    TASKBAR_EDGE: Setting = "taskbarEdge" => "bottom",
    // The top bar and the dock (display split, island.rs is_split): the
    // top bar 32 px or 48, the dock out of the way of a window touching
    // it or kept in the room Windows' own keeps, quick settings and the
    // clock in the top bar or the dock.
    SPLIT_BAR: Setting = "splitBar" => 32,
    SPLIT_DOCK: Setting = "splitDock" => "hide",
    SPLIT_KEYS: Setting = "splitKeys" => "top",
    // OLED's guard, on unless turned off: the island's and the taskbar's
    // icons and words a screen pixel off by turns (island.ts oledShift).
    "oledShift" => true,
    // Her updates looked for as she starts and once a day (updates.rs;
    // a downloaded exe only); downloaded from Gitee first in China and
    // GitHub first elsewhere, or as set.
    AUTO_UPDATE_CHECK: Setting = "autoUpdateCheck" => true,
    UPDATE_FROM: Setting = "updateFrom" => "auto",
    DETAILS: Setting = "details" => true,
    // Widgets (plugins in the island): which are off, their order, how
    // often the island turns to the next, whether they may open it.
    WIDGETS_OFF: Setting = "widgetsOff" => ["monitor"],
    // The monitor's parts, and how often it reads (seconds, 1 to 10):
    // the CPU, memory and network's up and down by default, nothing more
    // (the user, 2026-10-16; Windows' own taskbar has the battery).
    MONITOR: Setting = "monitor" => { "cpu": true, "mem": true, "net": true, "battery": false, "every": 2, "pin": true },
    WIDGET_ORDER: Setting = "widgetOrder" => [],
    "widgetSpin" => 8,
    WIDGET_NUDGE: Setting = "widgetNudge" => true,
    // The plugins she runs (scripts.rs): { id: { on, <Param>: value } }.
    PLUGINS: Setting = "plugins" => {},
    // Mail accounts (mail.rs); their passwords are in the Credential Manager.
    MAIL: Setting = "mail" => [],
    // A code or a link to sign in with, come by mail, on the island (mail/codes.rs).
    MAIL_CODES: Setting = "mailCodes" => true,
    WALK: Flag = "walk" => true,
    LOOK: Flag = "look" => true,
    // Done, changed or failed: shown 8 s, or until seen ("seen"); 30 and
    // 120 the other quick choices, any whole seconds from 3 to 600 too
    // (settings.rs is_hold, #59).
    HOLD: Setting = "hold" => 8,
    // At each, and whenever she has something to tell that needs no
    // answer (a hello, a plugin's nudge): round her closed island a ring
    // of colour, then a dot until seen (ring, the flow light: island.ts
    // cue); the island open by itself for a moment (open, all as before
    // 2026-10-16); or the dot alone (none). A session waiting on you
    // opens it whatever this says.
    "endCue" => "ring",
    // A session at rest out of sight after this many minutes with nothing
    // new from it (15, 30, 60, 120; 0 never: state.rs is_shown).
    IDLE_HIDE_MIN: Setting = "idleHideMin" => (crate::state::IDLE_HIDE_MIN),
    NOTIFY: Setting = "notify" => { "waiting": false, "done": false, "error": false },
    "sound" => false,
    PROMPT_WAIT_SEC: Setting = "promptWaitSec" => 290,
    DND: Flag = "dnd" => false,
    HIDE_IN_FULLSCREEN: Flag = "hideInFullscreen" => true,
    ONBOARDED: Flag = "onboarded" => false,
    // Global shortcuts for prompt approval and island toggle (shortcuts.rs)
    SHORTCUTS_ENABLED: Flag = "shortcutsEnabled" => false,
    SHORTCUT_APPROVE: Setting = "shortcutApprove" => "Alt+Y",
    SHORTCUT_DENY: Setting = "shortcutDeny" => "Alt+N",
    SHORTCUT_ISLAND: Setting = "shortcutIsland" => "Alt+W",
    // Which defaults a config was written under (load: one from before
    // 2026-10-13 keeps the look it had).
    "lookDefaults" => 2,
    // Read here, with no default: set by the pages or by her as she goes.
    CONNECT_HINT_OFF: Flag = "connectHintOff",
    CRASH_TOLD: Setting = "crashTold",
    DESK_LAYOUT: Setting = "deskLayout",
    DSH_VIA: Setting = "dshVia",
    ISLAND_LOOK: Setting = "islandLook",
    ISLAND_TOP_LOOK: Setting = "islandTopLook",
    MAIL_AGENT_CONF: Setting = "mailAgentConf",
    MAIL_OFFLINE: Setting = "mailOffline",
    MAIL_PROXY: Setting = "mailProxy",
    MONITOR_SOURCE: Setting = "monitorSource",
    NET_PROXY: Setting = "netProxy",
    PLUGIN_AUTO_UPDATE: Setting = "pluginAutoUpdate",
    SETTINGS_PIN: Flag = "settingsPin",
    SETTINGS_TOP: Flag = "settingsTop",
    TASKBAR_LAYOUT: Setting = "taskbarLayout",
    TASKBAR_ORDER: Setting = "taskbarOrder",
    TASKBAR_PINNED: Setting = "taskbarPinned",
    TASKBAR_PINS_IMPORTED: Flag = "taskbarPinsImported",
    TRAY_PINNED: Setting = "trayPinned",
    UPDATE_TOLD: Setting = "updateTold",
    WELCOME_OPENED: Flag = "welcomeOpened",
    X: Setting = "x",
    Y: Setting = "y",
}

#[cfg(test)]
mod tests {
    use super::*;

    // The defaults as data.rs had them before they were declared here
    // (2026-11-12): a new config starts the same.
    #[test]
    fn the_defaults_are_as_they_were() {
        let Value::Object(was) = json!({
            "lang": "auto",
            "pet": "claude-chan",
            "scale": 0.55,
            // Her home: island or taskbar (the taskbar from the first start, on
            // Windows; elsewhere there is no taskbar of hers, only the island);
            // out, she is on the desktop.
            "display": if cfg!(windows) { "taskbar" } else { "island" },
            "out": false,
            // The compact island's width: narrow, normal or wide (island.ts WIDTHS).
            "islandWidth": "normal",
            // Where it sits at the top of the screen: left, center or right
            // (anchor.ts, zhoupengjie/Wakuwaku#59); the top centre as ever.
            "islandPos": "center",
            // Where it is: top (the top of the screen, as ever), or taskbar, in
            // Windows' own taskbar along whichever edge that is, which stays as
            // it is (island.rs fit, zhoupengjie/Wakuwaku#105).
            "islandEdge": "top",
            // In the taskbar, her capsule inside it, taking one end of it
            // (embed.rs), unless turned off: laid over it then.
            "islandEmbed": true,
            // At the top of the screen, slid up out of the way while a window
            // lies under it (island-plate.ts setStow, zhoupengjie/Wakuwaku#49).
            "islandHideUnder": true,
            // The taskbar's strip: mica (the desktop's picture through it, mica.ts),
            // black (solid: black, or light in Windows' light mode) or clear (the
            // desktop itself through it, a new user's since 2026-10-13); clear,
            // Mica or solid while a window is maximized or full screen on its
            // display (mica, solid), or clear always.
            "taskbarMaterial": "clear",
            // The display her home is on (zhoupengjie/Wakuwaku#104): a display's
            // key (screen.rs), or "" for Windows' main display, as ever.
            "homeDisplay": "",
            // Her taskbar on her display alone (no taskbar on the others)
            // or on all of them (bars.rs).
            "taskbarDisplays": "main",
            "taskbarClearWhen": "mica",
            // Its programs' buttons: icons alone or with their titles; in the
            // middle of the screen or after her end (island-layout.ts alignApps).
            "taskbarButtons": "icons",
            "taskbarAlign": "center",
            // The screen's edge it is along: bottom (Windows' own place), top,
            // left or right (side bars: an upright capsule, icons alone).
            "taskbarEdge": "bottom",
            // The top bar and the dock (display split, island.rs is_split): the
            // top bar 32 px or 48, the dock out of the way of a window touching
            // it or kept in the room Windows' own keeps, quick settings and the
            // clock in the top bar or the dock.
            "splitBar": 32,
            "splitDock": "hide",
            "splitKeys": "top",
            // OLED's guard, on unless turned off: the island's and the taskbar's
            // icons and words a screen pixel off by turns (island.ts oledShift).
            "oledShift": true,
            // Her updates looked for as she starts and once a day (updates.rs;
            // a downloaded exe only); downloaded from Gitee first in China and
            // GitHub first elsewhere, or as set.
            "autoUpdateCheck": true,
            "updateFrom": "auto",
            "details": true,
            // Widgets (plugins in the island): which are off, their order, how
            // often the island turns to the next, whether they may open it.
            "widgetsOff": ["monitor"],
            // The monitor's parts, and how often it reads (seconds, 1 to 10):
            // the CPU, memory and network's up and down by default, nothing more
            // (the user, 2026-10-16; Windows' own taskbar has the battery).
            "monitor": { "cpu": true, "mem": true, "net": true, "battery": false, "every": 2, "pin": true },
            "widgetOrder": [],
            "widgetSpin": 8,
            "widgetNudge": true,
            // The plugins she runs (scripts.rs): { id: { on, <Param>: value } }.
            "plugins": {},
            // Mail accounts (mail.rs); their passwords are in the Credential Manager.
            "mail": [],
            // A code or a link to sign in with, come by mail, on the island (mail/codes.rs).
            "mailCodes": true,
            "walk": true,
            "look": true,
            // Done, changed or failed: shown 8 s, or until seen ("seen"); 30 and
            // 120 the other quick choices, any whole seconds from 3 to 600 too
            // (settings.rs is_hold, #59).
            "hold": 8,
            // At each, and whenever she has something to tell that needs no
            // answer (a hello, a plugin's nudge): round her closed island a ring
            // of colour, then a dot until seen (ring, the flow light: island.ts
            // cue); the island open by itself for a moment (open, all as before
            // 2026-10-16); or the dot alone (none). A session waiting on you
            // opens it whatever this says.
            "endCue": "ring",
            // A session at rest out of sight after this many minutes with nothing
            // new from it (15, 30, 60, 120; 0 never: state.rs is_shown).
            "idleHideMin": crate::state::IDLE_HIDE_MIN,
            "notify": { "waiting": false, "done": false, "error": false },
            "sound": false,
            "promptWaitSec": 290,
            "dnd": false,
            "hideInFullscreen": true,
            "onboarded": false,
            // Global shortcuts for prompt approval and island toggle (shortcuts.rs)
            "shortcutsEnabled": false,
            "shortcutApprove": "Alt+Y",
            "shortcutDeny": "Alt+N",
            "shortcutIsland": "Alt+W",
            // Which defaults a config was written under (load: one from before
            // 2026-10-13 keeps the look it had).
            "lookDefaults": 2,
        }) else {
            unreachable!()
        };
        assert_eq!(defaults(), was);
    }

    #[test]
    fn each_key_once() {
        let mut names = NAMES.to_vec();
        names.sort_unstable();
        let all = names.len();
        names.dedup();
        assert_eq!(names.len(), all, "a key declared twice");
    }

    // A switch is on when true alone, as Shared::flag always read it.
    #[test]
    fn a_switch_is_on_when_true_alone() {
        assert!(is_on(&json!(true)));
        for v in [json!(false), Value::Null, json!(1), json!("true"), json!({})] {
            assert!(!is_on(&v), "{v}");
        }
    }

    // The kinds keep their names, and a constant is its key in the file.
    #[test]
    fn a_constant_is_its_key() {
        assert_eq!(DND.name(), "dnd");
        assert_eq!(DISPLAY.name(), "display");
        assert_eq!(Key::name(TASKBAR_PINS_IMPORTED), "taskbarPinsImported");
        assert!(defaults().contains_key("islandWidth"));
    }
}
