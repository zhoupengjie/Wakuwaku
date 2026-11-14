// Her settings as the Rust side reads them (config.json in her data folder,
// data.rs load and save): each key declared here once, with the field it
// is read by, its kind and its default. Read from a snapshot (Config, an
// Arc taken from Shared::config, the lock let go at once): a field each,
// of its kind's type, never a key typed as a string where it is read, so
// one misspelt is a compile error, not a setting quietly never found. The
// kinds tell how a value is read: a switch (Flag: on only when true; On:
// on unless false), a string, a number, a list of strings, or the JSON as
// it is (Raw: Null while unset). Each reader still says what it makes of a
// missing or odd value (a default of its own), as it did.
//
// The file itself is as it was: a JSON object of every key, kept whole in
// the snapshot (Config::map) and written back so, a key the pages write
// that is not declared here (theirs alone) with the rest, and a value not
// of its key's kind (a hand-edited file) as it was; the pages get that
// object as ever. src/shared/config.ts's declared keys are written from
// here (tests::the_pages_config_is_written_from_here).
use serde_json::{json, Map, Value};

// A key, by its name in the file: for a patch's keys (Shared::change) and
// the rare reader that needs the JSON itself (Config::raw).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Key(&'static str);

impl Key {
    pub const fn name(self) -> &'static str {
        self.0
    }
}

// What the pages may set a key to (a settings patch: settings.rs
// apply_patch), said with the key below. Until 2026-11-14 a match on the
// keys' names in settings.rs, apart from where the keys are declared.
pub enum Check {
    // true or false.
    Bool,
    // One of these words.
    OneOf(&'static [&'static str]),
    // One of these whole numbers.
    UIntOf(&'static [u64]),
    // Whatever this passes: a layout, a list, a thing with parts.
    With(fn(&Value) -> bool),
    // One that has to ask her (the pets there are, the shortcuts taken):
    // her, the key, the value.
    Asking(fn(&crate::Shared, &str, &Value) -> bool),
}

impl Check {
    // Whether a value passes, for one that need not ask her (None: it must).
    pub fn passes_alone(&self, v: &Value) -> Option<bool> {
        Some(match self {
            Check::Bool => v.is_boolean(),
            Check::OneOf(words) => v.as_str().is_some_and(|s| words.contains(&s)),
            Check::UIntOf(numbers) => v.as_u64().is_some_and(|n| numbers.contains(&n)),
            Check::With(passes) => passes(v),
            Check::Asking(_) => return None,
        })
    }

    pub fn passes(&self, sh: &crate::Shared, key: &str, v: &Value) -> bool {
        match self {
            Check::Asking(passes) => passes(sh, key, v),
            _ => self.passes_alone(v).unwrap_or(false),
        }
    }
}

// A key's check, if the pages may set it.
pub fn check_of(key: &str) -> Option<&'static Check> {
    CHECKS.iter().find(|(name, _)| *name == key).map(|(_, check)| check)
}

// A switch's value as a Flag reads it.
pub fn is_on(value: &Value) -> bool {
    value.as_bool().unwrap_or(false)
}

// How a key's value is read, the type it is read as, and its TypeScript
// type for the pages' Config ("" for Raw: by its default).
pub trait Kind {
    type Out;
    const TS: &'static str;
    fn read(v: &Value) -> Self::Out;
}

// On only when it is true (absent, null or anything else, off).
pub struct Flag;
// On unless it is false (absent, null, anything else, on).
pub struct On;
// A string, if it is one.
pub struct Text;
// A number, if it is one.
pub struct Num;
// A whole number at least 0, if it is one.
pub struct UInt;
// A whole number, if it is one.
pub struct Int;
// true or false, if it is either.
pub struct YesNo;
// The strings of a list (anything else in it left out; not a list, none).
pub struct List;
// The JSON as it is: a list, an object; Null while unset.
pub struct Raw;

impl Kind for Flag {
    type Out = bool;
    const TS: &'static str = "boolean";
    fn read(v: &Value) -> bool {
        is_on(v)
    }
}

impl Kind for On {
    type Out = bool;
    const TS: &'static str = "boolean";
    fn read(v: &Value) -> bool {
        *v != Value::Bool(false)
    }
}

impl Kind for Text {
    type Out = Option<String>;
    const TS: &'static str = "string";
    fn read(v: &Value) -> Option<String> {
        v.as_str().map(str::to_string)
    }
}

impl Kind for Num {
    type Out = Option<f64>;
    const TS: &'static str = "number";
    fn read(v: &Value) -> Option<f64> {
        v.as_f64()
    }
}

impl Kind for UInt {
    type Out = Option<u64>;
    const TS: &'static str = "number";
    fn read(v: &Value) -> Option<u64> {
        v.as_u64()
    }
}

impl Kind for Int {
    type Out = Option<i64>;
    const TS: &'static str = "number";
    fn read(v: &Value) -> Option<i64> {
        v.as_i64()
    }
}

impl Kind for YesNo {
    type Out = Option<bool>;
    const TS: &'static str = "boolean";
    fn read(v: &Value) -> Option<bool> {
        v.as_bool()
    }
}

impl Kind for List {
    type Out = Vec<String>;
    const TS: &'static str = "";
    fn read(v: &Value) -> Vec<String> {
        v.as_array().into_iter().flatten().filter_map(|s| s.as_str().map(str::to_string)).collect()
    }
}

impl Kind for Raw {
    type Out = Value;
    const TS: &'static str = "";
    fn read(v: &Value) -> Value {
        v.clone()
    }
}

// The keys: `NAME field: Kind = "key" => default` (read here, with a
// default), `NAME field: Kind = "key"` (read here, none: unset till the
// pages or she set it), `"key" => default` (a default for the pages, never
// read here). `; "ts type"` after one gives its TypeScript type, where its
// kind's is not the pages'. `set <Check>` last says what the pages may set
// it to (settings.rs apply_patch); a key with none is not theirs to set.
macro_rules! keys {
    ($( $($name:ident $field:ident: $kind:ident =)? $key:literal $(=> $default:tt)? $(; $ts:literal)? $(set $check:expr)?, )*) => {
        $( $( #[allow(dead_code)] pub const $name: Key = Key($key); )? )*

        // Every setting with a default, as a new config starts (data.rs load).
        pub fn defaults() -> Map<String, Value> {
            let mut map = Map::new();
            $( $( map.insert($key.to_string(), json!($default)); )? )*
            map
        }

        // Her settings as they are now: the file's object whole (map), and
        // each key read here as its kind reads it. Made anew on each change
        // (Shared::change), never changed in place.
        pub struct Config {
            map: Map<String, Value>,
            $( $( pub $field: <$kind as Kind>::Out, )? )*
        }

        impl Config {
            pub fn from_map(map: Map<String, Value>) -> Config {
                Config {
                    $( $( $field: <$kind as Kind>::read(map.get($key).unwrap_or(&Value::Null)), )? )*
                    map,
                }
            }
        }

        // What the pages may set each key to (Check): those said with a
        // key (`set ...`), in the order declared. A key not among them
        // is not theirs to set.
        pub const CHECKS: &[(&str, Check)] = {
            use Check::*;
            &[$( $( ($key, $check), )? )*]
        };

        // Every key declared, with a default or not.
        #[cfg(test)]
        pub const NAMES: &[&str] = &[$($key),*];

        // The pages' Config's declared keys, a line each, in this order
        // (src/shared/config.ts): its kind's type, else by its default.
        #[cfg(test)]
        #[allow(unused_variables)]
        pub fn ts_lines() -> Vec<String> {
            let mut lines = Vec::new();
            $({
                let kind: &str = "";
                $( let kind = <$kind as Kind>::TS; )?
                let default: Option<Value> = None;
                $( let default = Some(json!($default)); )?
                let ts: Option<&str> = None;
                $( let ts = Some($ts); )?
                let check: Option<Check> = None;
                $( let check = Some({ use Check::*; $check }); )?
                lines.push(format!("  {}?: {}", $key, ts_type(kind, default.as_ref(), ts, check.as_ref())));
            })*
            lines
        }
    };
}

#[cfg(test)]
fn ts_type(kind: &str, default: Option<&Value>, ts: Option<&str>, check: Option<&Check>) -> String {
    if let Some(ts) = ts {
        return ts.to_string();
    }
    // One of a few words: those words.
    if let Some(Check::OneOf(words)) = check {
        return words.iter().map(|w| format!("'{w}'")).collect::<Vec<_>>().join(" | ");
    }
    if !kind.is_empty() {
        return kind.to_string();
    }
    match default {
        Some(Value::String(_)) => "string",
        Some(Value::Number(_)) => "number",
        Some(Value::Bool(_)) => "boolean",
        Some(Value::Array(_)) => "any[]",
        Some(Value::Object(_)) => "Record<string, any>",
        _ => "any",
    }
    .to_string()
}

impl Config {
    // The file's object, every key as it is (the pages', data.rs save).
    pub fn map(&self) -> &Map<String, Value> {
        &self.map
    }

    // A key's JSON as it is, Null while unset: for one handed on whole.
    pub fn raw(&self, key: Key) -> &Value {
        self.map.get(key.name()).unwrap_or(&Value::Null)
    }

    // Her home as her windows have it: the island, or the taskbar, which
    // the top bar and the dock are too (display split: island/ is_split).
    pub fn home(&self) -> &'static str {
        match self.display.as_deref() {
            Some("taskbar" | "split") => "taskbar",
            _ => "island",
        }
    }
}

impl Default for Config {
    fn default() -> Config {
        Config::from_map(defaults())
    }
}

keys! {
    LANG lang: Text = "lang" => "auto" set OneOf(&["auto", "zh", "en"]),
    PET pet: Raw = "pet" => "claude-chan" set Asking(crate::settings::is_pet),
    SCALE scale: Num = "scale" => 0.55 set With(crate::settings::is_scale),
    // Her home: island or taskbar (the taskbar from the first start, on
    // Windows; elsewhere there is no taskbar of hers, only the island);
    // out, she is on the desktop.
    DISPLAY display: Text = "display" => (if cfg!(windows) { "taskbar" } else { "island" }) set With(crate::settings::is_display),
    OUT out: Flag = "out" => false set Bool,
    // The compact island's width: narrow, normal or wide (island.ts WIDTHS).
    "islandWidth" => "normal" set OneOf(&["narrow", "normal", "wide"]),
    // Where it sits at the top of the screen: left, center or right
    // (anchor.ts, zhoupengjie/Wakuwaku#59); the top centre as ever.
    ISLAND_POS island_pos: Text = "islandPos" => "center" set With(crate::settings::is_island_pos),
    // Where it is: top (the top of the screen, as ever), or taskbar, in
    // Windows' own taskbar along whichever edge that is, which stays as
    // it is (island/ fit, zhoupengjie/Wakuwaku#105).
    ISLAND_EDGE island_edge: Text = "islandEdge" => "top" set With(crate::settings::is_island_edge),
    // In the taskbar, her capsule inside it, taking one end of it
    // (embed.rs), unless turned off: laid over it then.
    ISLAND_EMBED island_embed: YesNo = "islandEmbed" => true set Bool,
    // At the top of the screen, slid up out of the way while a window
    // lies under it (island-plate.ts setStow, zhoupengjie/Wakuwaku#49).
    ISLAND_HIDE_UNDER island_hide_under: Flag = "islandHideUnder" => true set Bool,
    // The taskbar's strip: mica (the desktop's picture through it, mica.ts),
    // black (solid: black, or light in Windows' light mode) or clear (the
    // desktop itself through it, a new user's since 2026-10-13); clear,
    // Mica or solid while a window is maximized or full screen on its
    // display (mica, solid), or clear always.
    TASKBAR_MATERIAL taskbar_material: Text = "taskbarMaterial" => "clear" set OneOf(&["mica", "black", "clear"]),
    // The display her home is on (zhoupengjie/Wakuwaku#104): a display's
    // key (screen.rs), or "" for Windows' main display, as ever.
    HOME_DISPLAY home_display: Raw = "homeDisplay" => "" set With(crate::settings::is_home_display),
    // Her taskbar on her display alone (no taskbar on the others)
    // or on all of them (bars.rs).
    TASKBAR_DISPLAYS taskbar_displays: Text = "taskbarDisplays" => "main" set OneOf(&["main", "all"]),
    TASKBAR_CLEAR_WHEN taskbar_clear_when: Raw = "taskbarClearWhen" => "mica" set OneOf(&["mica", "solid", "always"]),
    // Its programs' buttons: icons alone or with their titles; in the
    // middle of the screen or after her end (island-layout.ts alignApps).
    "taskbarButtons" => "icons" set OneOf(&["icons", "labels"]),
    TASKBAR_ALIGN taskbar_align: Raw = "taskbarAlign" => "center" set OneOf(&["center", "left"]),
    // The screen's edge it is along: bottom (Windows' own place), top,
    // left or right (side bars: an upright capsule, icons alone).
    TASKBAR_EDGE taskbar_edge: Text = "taskbarEdge" => "bottom" set OneOf(&["bottom", "top", "left", "right"]),
    // The top bar and the dock (display split, island/ is_split): the
    // top bar 32 px or 48, the dock out of the way of a window touching
    // it or kept in the room Windows' own keeps, quick settings and the
    // clock in the top bar or the dock.
    SPLIT_BAR split_bar: UInt = "splitBar" => 32 set UIntOf(&[32, 48]),
    SPLIT_DOCK split_dock: Text = "splitDock" => "hide" set OneOf(&["hide", "room"]),
    SPLIT_KEYS split_keys: Text = "splitKeys" => "top" set OneOf(&["top", "dock"]),
    // OLED's guard, on unless turned off: the island's and the taskbar's
    // icons and words a screen pixel off by turns (island.ts oledShift).
    "oledShift" => true set Bool,
    // Her updates looked for as she starts and once a day (updates.rs;
    // a downloaded exe only); downloaded from Gitee first in China and
    // GitHub first elsewhere, or as set.
    AUTO_UPDATE_CHECK auto_update_check: On = "autoUpdateCheck" => true set Bool,
    UPDATE_FROM update_from: Text = "updateFrom" => "auto" set OneOf(&["auto", "github", "gitee"]),
    DETAILS details: On = "details" => true set Bool,
    // Widgets (plugins in the island): which are off, their order, how
    // often the island turns to the next, whether they may open it.
    WIDGETS_OFF widgets_off: List = "widgetsOff" => ["monitor"] set With(crate::settings::is_widget_ids),
    // The monitor's parts, and how often it reads (seconds, 1 to 10):
    // the CPU, memory and network's up and down by default, nothing more
    // (the user, 2026-10-16; Windows' own taskbar has the battery).
    MONITOR monitor: Raw = "monitor" => { "cpu": true, "mem": true, "net": true, "battery": false, "every": 2, "pin": true } set With(crate::settings::is_monitor),
    WIDGET_ORDER widget_order: List = "widgetOrder" => [] set With(crate::settings::is_widget_ids),
    "widgetSpin" => 8 set UIntOf(&[0, 5, 8, 15]),
    WIDGET_NUDGE widget_nudge: On = "widgetNudge" => true set Bool,
    // The plugins she runs (scripts.rs): { id: { on, <Param>: value } }.
    PLUGINS plugins: Raw = "plugins" => {} set With(crate::plugins::is_ok),
    // Mail accounts (mail.rs); their passwords are in the Credential Manager.
    MAIL mail: Raw = "mail" => [],
    // A code or a link to sign in with, come by mail, on the island (mail/codes.rs).
    MAIL_CODES mail_codes: On = "mailCodes" => true set Bool,
    WALK walk: Flag = "walk" => true set Bool,
    LOOK look: Flag = "look" => true set Bool,
    // Done, changed or failed: shown 8 s, or until seen ("seen"); 30 and
    // 120 the other quick choices, any whole seconds from 3 to 600 too
    // (settings.rs is_hold, #59).
    HOLD hold: UInt = "hold" => 8; "number | 'seen'" set With(crate::settings::is_hold),
    // At each, and whenever she has something to tell that needs no
    // answer (a hello, a plugin's nudge): round her closed island a ring
    // of colour, then a dot until seen (ring, the flow light: island.ts
    // cue); the island open by itself for a moment (open, all as before
    // 2026-10-16); or the dot alone (none). A session waiting on you
    // opens it whatever this says.
    "endCue" => "ring" set OneOf(&["open", "ring", "none"]),
    // A session at rest out of sight after this many minutes with nothing
    // new from it (15, 30, 60, 120; 0 never: state.rs is_shown).
    IDLE_HIDE_MIN idle_hide_min: UInt = "idleHideMin" => (crate::agents::state::IDLE_HIDE_MIN) set UIntOf(&[0, 15, 30, 60, 120]),
    NOTIFY notify: Raw = "notify" => { "waiting": false, "done": false, "error": false } set With(crate::settings::is_notify),
    "sound" => false set Bool,
    PROMPT_WAIT_SEC prompt_wait_sec: UInt = "promptWaitSec" => 290 set UIntOf(&[30, 60, 120, 290]),
    // Approvals other programs on this computer ask for, on her prompt
    // panel (app_asks.rs, POST /ask); off, they are told so at once.
    APP_ASKS app_asks: On = "appAsks" => true set Bool,
    DND dnd: Flag = "dnd" => false set Bool,
    HIDE_IN_FULLSCREEN hide_in_fullscreen: Flag = "hideInFullscreen" => true set Bool,
    ONBOARDED onboarded: Flag = "onboarded" => false set Bool,
    // Global shortcuts for prompt approval and island toggle (shortcuts.rs)
    SHORTCUTS_ENABLED shortcuts_enabled: Flag = "shortcutsEnabled" => false set Bool,
    SHORTCUT_APPROVE shortcut_approve: Text = "shortcutApprove" => "Alt+Y" set Asking(crate::settings::is_shortcut),
    SHORTCUT_DENY shortcut_deny: Text = "shortcutDeny" => "Alt+N" set Asking(crate::settings::is_shortcut),
    SHORTCUT_ISLAND shortcut_island: Text = "shortcutIsland" => "Alt+W" set Asking(crate::settings::is_shortcut),
    // Which defaults a config was written under (load: one from before
    // 2026-10-13 keeps the look it had).
    "lookDefaults" => 2,
    // Read here, with no default: set by the pages or by her as she goes.
    CONNECT_HINT_OFF connect_hint_off: Flag = "connectHintOff" set Bool,
    CRASH_TOLD crash_told: Text = "crashTold",
    // Crash reports sent: "ask" (as she first is), "auto" or "off" (crash.rs).
    CRASH_REPORTS crash_reports: Text = "crashReports" set OneOf(&["auto", "off", "ask"]),
    DESK_LAYOUT desk_layout: Raw = "deskLayout" set With(crate::settings::is_desk_layout),
    DSH_VIA dsh_via: Raw = "dshVia" set OneOf(&["plugin", "bridge"]),
    ISLAND_LOOK island_look: Raw = "islandLook" set With(crate::settings::is_island_looks),
    ISLAND_TOP_LOOK island_top_look: Raw = "islandTopLook" set With(crate::settings::is_island_looks),
    MAIL_AGENT_CONF mail_agent_conf: Raw = "mailAgentConf" set With(crate::mail::agent::is_conf_ok),
    MAIL_OFFLINE mail_offline: Raw = "mailOffline" set OneOf(&["90d", "year", "all", "opened"]),
    MAIL_PROXY mail_proxy: Raw = "mailProxy" set With(crate::mail::proxy::is_page_way),
    MONITOR_SOURCE monitor_source: Raw = "monitorSource" set With(crate::hw::is_source),
    NET_PROXY net_proxy: Raw = "netProxy" set With(crate::settings::is_net_proxy),
    PLUGIN_AUTO_UPDATE plugin_auto_update: On = "pluginAutoUpdate" set Bool,
    SETTINGS_PIN settings_pin: Flag = "settingsPin" set Bool,
    SETTINGS_TOP settings_top: Flag = "settingsTop" set Bool,
    TASKBAR_LAYOUT taskbar_layout: Raw = "taskbarLayout" set With(crate::settings::is_taskbar_layout_or_none),
    TASKBAR_ORDER taskbar_order: List = "taskbarOrder"; "any" set With(crate::settings::is_taskbar_order),
    TASKBAR_PINNED taskbar_pinned: Raw = "taskbarPinned",
    TASKBAR_PINS_IMPORTED taskbar_pins_imported: Flag = "taskbarPinsImported",
    TRAY_PINNED tray_pinned: Raw = "trayPinned",
    UPDATE_TOLD update_told: Text = "updateTold",
    WELCOME_OPENED welcome_opened: Flag = "welcomeOpened",
    X x: Int = "x",
    Y y: Int = "y",
    // Set by the pages and never read here: declared for what they may be
    // set to. Her capsule's members, their order and how each shows there
    // (capsule.ts).
    "capsule" set With(crate::settings::is_capsule),
    // Boards kept as combos, to be put up again from the picker's 组合
    // (deskgrid.ts setsOf).
    "boardSets" set With(crate::settings::is_board_sets),
    // Each widget's row in the open island, small or medium, by its id
    // (island.ts rowSize; addons/ drops a plugin's when it goes).
    "widgetSize"; "Record<string, string>" set With(crate::settings::is_widget_sizes),
    // The agent a letter is handed to (mail/agent.rs).
    "mailAgent" set OneOf(&["claude", "codex", "agy"]),
    // The Mail page in two panes (list, letter), or one.
    "mailPanes" set OneOf(&["one", "two"]),
    // The senders whose letters show their pictures from elsewhere
    // (settings-letter.ts): their addresses, in lower case.
    "mailPictures"; "string[]" set With(crate::settings::is_addresses),
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
            // it is (island/ fit, zhoupengjie/Wakuwaku#105).
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
            // The top bar and the dock (display split, island/ is_split): the
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
            "idleHideMin": crate::agents::state::IDLE_HIDE_MIN,
            "notify": { "waiting": false, "done": false, "error": false },
            "sound": false,
            "promptWaitSec": 290,
            "appAsks": true,
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

    // A switch is on when true alone, as a Flag reads it (and Shared::flag did).
    #[test]
    fn a_switch_is_on_when_true_alone() {
        assert!(is_on(&json!(true)));
        for v in [json!(false), Value::Null, json!(1), json!("true"), json!({})] {
            assert!(!is_on(&v), "{v}");
        }
    }

    // A constant is its key in the file.
    #[test]
    fn a_constant_is_its_key() {
        assert_eq!(DND.name(), "dnd");
        assert_eq!(DISPLAY.name(), "display");
        assert_eq!(TASKBAR_PINS_IMPORTED.name(), "taskbarPinsImported");
        assert!(defaults().contains_key("islandWidth"));
    }

    // Each kind reads as the readers did with the JSON: Flag as is_on, On
    // as `!= false`, the rest as as_str, as_f64, as_u64, as_i64, as_bool.
    #[test]
    fn the_kinds_read_as_the_readers_did() {
        for v in [json!(true), json!(false), Value::Null, json!(1), json!("x"), json!([1]), json!({})] {
            assert_eq!(Flag::read(&v), is_on(&v), "{v}");
            assert_eq!(On::read(&v), v != false, "{v}");
            assert_eq!(Text::read(&v).as_deref(), v.as_str(), "{v}");
            assert_eq!(Num::read(&v), v.as_f64(), "{v}");
            assert_eq!(UInt::read(&v), v.as_u64(), "{v}");
            assert_eq!(Int::read(&v), v.as_i64(), "{v}");
            assert_eq!(YesNo::read(&v), v.as_bool(), "{v}");
            assert_eq!(Raw::read(&v), v, "{v}");
        }
        assert_eq!(List::read(&json!(["a", 1, "b", null])), vec!["a".to_string(), "b".to_string()]);
        assert!(List::read(&json!("a")).is_empty());
        assert_eq!(UInt::read(&json!(-1)), None);
    }

    // The snapshot keeps the file's object whole: a key nobody declared,
    // and a value not of its key's kind, as they were (written back and
    // given to the pages so); each field read from it.
    #[test]
    fn the_snapshot_keeps_the_object_whole() {
        let mut map = defaults();
        map.insert("capsule".into(), json!({ "members": ["a"] }));
        map.insert("dnd".into(), json!("yes"));
        map.insert("scale".into(), json!(0.8));
        map.insert("x".into(), json!(-12));
        let config = Config::from_map(map.clone());
        assert_eq!(config.map(), &map);
        assert_eq!(serde_json::to_string_pretty(config.map()).unwrap(), serde_json::to_string_pretty(&map).unwrap());
        assert!(!config.dnd, "\"yes\" is no switch's true");
        assert_eq!(config.raw(DND), &json!("yes"));
        assert_eq!(config.scale, Some(0.8));
        assert_eq!((config.x, config.y), (Some(-12), None));
        assert_eq!(config.lang.as_deref(), Some("auto"));
        assert_eq!(config.widgets_off, vec!["monitor".to_string()]);
        assert_eq!(config.raw(Key("nothing")), &Value::Null);
        assert_eq!(Config::default().home(), if cfg!(windows) { "taskbar" } else { "island" });
    }

    // The keys declared that the pages may not set: hers alone to write
    // (where she is, what she has told, what she took from Windows), or
    // set by a command of their own (the mail accounts: mail_save). One
    // more here is a key that lost its check, or a new one without.
    #[test]
    fn the_keys_the_pages_may_not_set() {
        let mut none: Vec<&str> = NAMES.iter().copied().filter(|key| check_of(key).is_none()).collect();
        none.sort_unstable();
        assert_eq!(none, ["crashTold", "lookDefaults", "mail", "taskbarPinned", "taskbarPinsImported", "trayPinned", "updateTold", "welcomeOpened", "x", "y"]);
        assert!(check_of("nothing").is_none());
        // Each check is a declared key's, once.
        assert_eq!(CHECKS.len() + none.len(), NAMES.len());
    }

    // Each kind of check passes what it says, and nothing near it. (That
    // they pass what settings.rs's match on the keys' names passed was
    // seen once, key by key over a hundred values, before that match went:
    // 2026-11-14.)
    #[test]
    fn the_checks_pass_what_they_say() {
        let passes = |key: &str, v: Value| check_of(key).unwrap().passes_alone(&v);
        assert_eq!(passes("dnd", json!(true)), Some(true));
        assert_eq!(passes("dnd", json!(false)), Some(true));
        for no in [json!("true"), json!(1), Value::Null] {
            assert_eq!(passes("dnd", no.clone()), Some(false), "{no}");
        }
        assert_eq!(passes("lang", json!("zh")), Some(true));
        for no in [json!("fr"), json!("ZH"), json!(""), json!(1), json!(["zh"])] {
            assert_eq!(passes("lang", no.clone()), Some(false), "{no}");
        }
        assert_eq!(passes("splitBar", json!(48)), Some(true));
        for no in [json!(40), json!(48.0), json!("48"), json!(-32)] {
            assert_eq!(passes("splitBar", no.clone()), Some(false), "{no}");
        }
        // One of its own (settings.rs is_hold).
        assert_eq!(passes("hold", json!("seen")), Some(true));
        assert_eq!(passes("hold", json!(2)), Some(false));
        // One that asks her: not to be said without her.
        assert_eq!(passes("pet", json!("claude-chan")), None);
        assert_eq!(passes("shortcutDeny", json!("Alt+N")), None);
        // A few words are the pages' type for it.
        assert_eq!(ts_type("string", None, None, Some(&Check::OneOf(&["a", "b"]))), "'a' | 'b'");
        assert_eq!(ts_type("string", None, Some("any"), Some(&Check::OneOf(&["a", "b"]))), "any");
        assert_eq!(ts_type("boolean", None, None, Some(&Check::Bool)), "boolean");
    }

    // src/shared/config.ts's declared keys are these, a line each, written
    // from here: run with CONFIG_TS_BLESS=1 to write them anew after a key
    // is declared, changed or dropped here, then look at git diff.
    #[test]
    fn the_pages_config_is_written_from_here() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/shared/config.ts");
        let text = std::fs::read_to_string(&path).unwrap();
        let nl = if text.contains("\r\n") { "\r\n" } else { "\n" };
        let (from, to) = ("  // --- Declared in config.rs (written from it: cargo test config, CONFIG_TS_BLESS=1) ---", "  // --- The pages' own ---");
        let start = text.find(from).expect("the declared keys' heading") + from.len();
        let end = text.find(to).expect("the pages' own heading");
        let block: String = ts_lines().iter().map(|l| format!("{nl}{l}")).collect::<String>() + nl;
        if std::env::var_os("CONFIG_TS_BLESS").is_some() {
            std::fs::write(&path, format!("{}{block}{}", &text[..start], &text[end..])).unwrap();
            return;
        }
        assert_eq!(&text[start..end], block, "src/shared/config.ts is not as config.rs declares: CONFIG_TS_BLESS=1 cargo test config");
    }
}
