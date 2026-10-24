// Where her settings and pets live, the settings themselves, and the pets
// found.
//
// Her folder: WAKUWAKU_USER_DATA (tests), else data/ in the project when run
// from this source tree, else wakuwaku-data beside the .exe (portable).
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

// The source tree she was built in (build.rs); none in a release asset,
// which carries no path of the machine that built it.
const SOURCE_TREE: Option<&str> = option_env!("WAKUWAKU_SOURCE_TREE");

// The sizes she comes in.
pub const SCALES: [(&str, f64); 3] = [("small", 0.4), ("medium", 0.55), ("large", 0.75)];

// The source tree's path this many levels up, then `rest`: no "..", which
// the asset protocol's scope would not match.
fn in_tree(up: usize, rest: &str) -> Option<PathBuf> {
    let tree = Path::new(SOURCE_TREE?);
    Some(tree.ancestors().nth(up).unwrap_or(tree).join(rest))
}

pub fn folder() -> PathBuf {
    if let Ok(dir) = std::env::var("WAKUWAKU_USER_DATA") {
        return dir.into();
    }
    if let Some(data) = in_tree(1, "data").filter(|_| from_source()) {
        return data;
    }
    let exe = std::env::current_exe().unwrap_or_default();
    exe.parent().map(|p| p.join("wakuwaku-data")).unwrap_or_else(|| "wakuwaku-data".into())
}

// Quit by the person's choice (her menu's Quit, from the tray or the
// taskbar; POST /quit, zhoupengjie/Wakuwaku#57): marked in her data folder,
// so that no session's hook (Claude Code's SessionStart, Codex's,
// Antigravity's, ZCode's, Reasonix's, omp's: starts.rs starts_her) starts her again (it
// did, at once: a resume, /clear and a compaction start a session too) until
// she is started otherwise: opened by hand, at login, by an update, which
// every start of hers clears. A crash leaves no mark: the hooks start her
// again then, as ever. POST /quit?wake=1, and the handover of an update or a
// restart (its new one up already), quit so that a session may start her:
// any mark taken away.
const QUIT_MARK: &str = "quit-by-choice";

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Quit {
    ByChoice,
    Wakeable,
}

// The mark as a quit leaves it (Shared::quit; `first`: no quit under way
// before it): one by choice marks, even after another began (the person's
// word stands); the first that may be woken clears, a later one leaves it.
pub fn note_quit(dir: &Path, how: Quit, first: bool, why: &str) {
    match how {
        Quit::ByChoice => mark_quit(dir, why),
        Quit::Wakeable if first => drop(clear_quit(dir)),
        Quit::Wakeable => {}
    }
}

fn mark_quit(dir: &Path, why: &str) {
    let _ = fs::write(dir.join(QUIT_MARK), format!("quit by choice ({why}): a session's hook does not start her until she is opened again\n"));
}

// The mark taken away (she is up: main.rs), and what it said, if there was one.
pub fn clear_quit(dir: &Path) -> Option<String> {
    let note = quit_note(dir);
    let _ = fs::remove_file(dir.join(QUIT_MARK));
    note
}

// What the mark says (its first line), if she was quit by choice.
pub fn quit_note(dir: &Path) -> Option<String> {
    let text = fs::read_to_string(dir.join(QUIT_MARK)).ok()?;
    Some(text.lines().next().unwrap_or("").trim().to_string())
}

// Whether a session's hook may start her (none answering on her port).
pub fn may_start(dir: &Path) -> bool {
    !dir.join(QUIT_MARK).exists()
}

// Run from the source tree it was built in (its target folder), not a
// downloaded exe: its data stays in the tree, and it updates by a build.
pub fn from_source() -> bool {
    SOURCE_TREE.is_some_and(|tree| std::env::current_exe().unwrap_or_default().starts_with(Path::new(tree).join("target")))
}

// Her exe where it is now. Windows goes on giving the path she was started
// from after her file is renamed (GetModuleFileName), so a rename of her
// own while she runs is told here (updates.rs: an update's old exe taking
// its name back, or kept as <name>.old.exe), for what starts her or names
// her (her Run value, the hooks, her guard, her next update).
static EXE: std::sync::Mutex<Option<PathBuf>> = std::sync::Mutex::new(None);

pub fn exe() -> std::io::Result<PathBuf> {
    match EXE.lock().unwrap_or_else(|e| e.into_inner()).clone() {
        Some(exe) => Ok(exe),
        None => std::env::current_exe(),
    }
}

pub fn exe_moved(to: PathBuf) {
    *EXE.lock().unwrap_or_else(|e| e.into_inner()) = Some(to);
}

// --- Settings ----------------------------------------------------------------------

fn defaults() -> Map<String, Value> {
    let Value::Object(map) = json!({
        "lang": "auto",
        "pet": "claude-chan",
        "scale": 0.55,
        // Her home: island or taskbar (the taskbar from the first start, on
        // Windows; elsewhere there is no taskbar of hers, only the island);
        // out, she is on the desktop.
        "display": if cfg!(windows) { "taskbar" } else { "island" },
        "out": false,
        // The compact island's width: narrow, normal or wide (island.js WIDTHS).
        "islandWidth": "normal",
        // Where it sits at the top of the screen: left, center or right
        // (anchor.js, zhoupengjie/Wakuwaku#59); the top centre as ever.
        "islandPos": "center",
        // At the top of the screen, slid up out of the way while a window
        // lies under it (island.js setStow, zhoupengjie/Wakuwaku#49).
        "islandHideUnder": true,
        // The taskbar's strip: mica (the desktop's picture through it, mica.js),
        // black (solid: black, or light in Windows' light mode) or clear (the
        // desktop itself through it, a new user's since 2026-10-13); clear,
        // Mica or solid while a window is maximized or full screen on its
        // display (mica, solid), or clear always.
        "taskbarMaterial": "clear",
        // Her taskbar on the main display alone (Windows' own on the others)
        // or on all of them (bars.rs).
        "taskbarDisplays": "main",
        "taskbarClearWhen": "mica",
        // Its programs' buttons: icons alone or with their titles; in the
        // middle of the screen or after her end (island.js alignApps).
        "taskbarButtons": "icons",
        "taskbarAlign": "center",
        // The screen's edge it is along: bottom (Windows' own place), top,
        // left or right (side bars: an upright capsule, icons alone).
        "taskbarEdge": "bottom",
        // OLED's guard, on unless turned off: the island's and the taskbar's
        // icons and words a screen pixel off by turns (island.js oledShift).
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
        "walk": true,
        "look": true,
        // Done, changed or failed: shown 8 s, or until seen ("seen"); 30 and
        // 120 the other quick choices, any whole seconds from 3 to 600 too
        // (settings.rs is_hold, #59).
        "hold": 8,
        // At each, and whenever she has something to tell that needs no
        // answer (a hello, a plugin's nudge): round her closed island a ring
        // of colour, then a dot until seen (ring, the flow light: island.js
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
    map
}

pub fn load(dir: &Path) -> Map<String, Value> {
    let mut settings = defaults();
    if let Ok(Value::Object(mut saved)) = fs::read_to_string(dir.join("config.json")).map_err(|_| ()).and_then(|t| serde_json::from_str(&t).map_err(|_| ())) {
        keep_old_look(&mut saved);
        adopt_end_cue(&mut saved);
        settings.extend(saved);
    }
    // The homes of earlier versions are the island now: the capsule, the
    // corner and the top bar; the pet on her own too, with her out of it.
    match settings.get("display").and_then(Value::as_str) {
        Some("pet") => {
            settings.insert("display".into(), json!("island"));
            settings.insert("out".into(), json!(true));
        }
        Some("capsule" | "corner" | "bar") => {
            settings.insert("display".into(), json!("island"));
        }
        // A settings file brought from Windows: no taskbar of hers here.
        Some("taskbar") if !cfg!(windows) => {
            settings.insert("display".into(), json!("island"));
        }
        _ => {}
    }
    settings.remove("corner");
    settings.remove("bubble");
    adopt_monitor(&mut settings);
    drop_part(&mut settings, "tags");
    // The gear, gone on 2026-10-14 (zhoupengjie/Wakuwaku#38), and its switch.
    drop_part(&mut settings, "gear");
    settings.remove("taskbarGear");
    settings
}

// A part the taskbar no longer has taken out of a layout kept with it, the
// rest kept as it was (the sessions' tags, gone from the taskbar on
// 2026-10-12, and the settings' gear on 2026-10-14: settings.rs would turn
// down the whole layout for either).
fn drop_part(settings: &mut Map<String, Value>, part: &str) {
    if let Some(Value::Object(runs)) = settings.get_mut("taskbarLayout") {
        for run in runs.values_mut() {
            if let Some(list) = run.as_array_mut() {
                list.retain(|p| p.as_str() != Some(part));
            }
        }
    }
}

// The defaults changed on 2026-10-13 (a new user's taskbar clear, and her
// island bare in the dark mode, the person's own look): a config written
// before keeps the look it had, once (lookDefaults then 2): Mica where none
// was chosen, and her island in the dark mode Windows' grey flyout, not
// bare, where it was the default.
fn keep_old_look(saved: &mut Map<String, Value>) {
    if saved.contains_key("lookDefaults") {
        return;
    }
    saved.insert("lookDefaults".into(), json!(2));
    saved.entry("taskbarMaterial").or_insert_with(|| json!("mica"));
    let looks = saved.entry("islandLook").or_insert(Value::Null);
    if !looks.is_object() {
        *looks = json!({});
    }
    if let Some(looks) = looks.as_object_mut() {
        if !looks.get("dark").is_some_and(Value::is_object) {
            looks.insert("dark".into(), json!({ "bg": "#2c2c2c", "op": 100, "blur": 0, "edge": "line", "bare": false }));
        }
    }
}

// The island opening by itself as a turn ended was a switch (openOnEnd, on
// by default) until 2026-10-16; now a choice of three for all she has to
// tell that needs no answer (endCue). One who had turned it off gets nothing
// but the dot (none); everyone else the new default, the ring. The old key goes.
fn adopt_end_cue(saved: &mut Map<String, Value>) {
    let Some(old) = saved.remove("openOnEnd") else { return };
    if !saved.contains_key("endCue") && old == false {
        saved.insert("endCue".into(), json!("none"));
    }
}

// CPU · memory, the network and the battery were widgets of their own: now
// they are the monitor's parts. It is on if any of them was, with those
// parts (all of them, if none was); it takes the first one's place in the order.
fn adopt_monitor(settings: &mut Map<String, Value>) {
    const OLD: [&str; 3] = ["sys", "net", "battery"];
    let list = |v: Option<&Value>| -> Vec<String> { v.and_then(Value::as_array).into_iter().flatten().filter_map(|s| s.as_str().map(String::from)).collect() };
    let off = list(settings.get("widgetsOff"));
    if !off.iter().any(|id| OLD.contains(&id.as_str())) {
        return;
    }
    let on = |id: &str| !off.iter().any(|o| o == id);
    let any = OLD.iter().any(|id| on(id));
    settings.insert("monitor".into(), json!({ "cpu": on("sys") || !any, "mem": on("sys") || !any, "net": on("net") || !any, "battery": on("battery") || !any, "every": 2 }));
    let mut off: Vec<String> = off.into_iter().filter(|id| !OLD.contains(&id.as_str())).collect();
    if !any {
        off.push("monitor".into());
    }
    settings.insert("widgetsOff".into(), json!(off));
    let mut order = Vec::new();
    for id in list(settings.get("widgetOrder")) {
        let id = if OLD.contains(&id.as_str()) { "monitor".to_string() } else { id };
        if !order.contains(&id) {
            order.push(id);
        }
    }
    settings.insert("widgetOrder".into(), json!(order));
}

pub fn save(dir: &Path, settings: &Map<String, Value>) {
    let _ = fs::create_dir_all(dir);
    // A settings file we cannot write only costs the remembered settings.
    let _ = fs::write(dir.join("config.json"), serde_json::to_string_pretty(settings).unwrap_or_default());
}

// --- Pets ----------------------------------------------------------------------------

#[derive(Clone)]
pub struct PetInfo {
    pub id: String,
    pub name: String,
    pub author: String,
    pub sheet: PathBuf,
    pub version: u8,
    // One of the pets Codex has (its desktop app keeps them in the same format).
    pub from_codex: bool,
    // Its sheet's time (ms), in its address: one put in anew (a pack's
    // update, a pet downloaded again) is loaded anew, not the old from the
    // page's cache.
    pub stamp: u64,
}

impl PetInfo {
    // Its sheet's address for the pages (asset_url, its time after it).
    pub fn url(&self) -> String {
        format!("{}?v={}", asset_url(&self.sheet), self.stamp)
    }
}

// The pets Codex has: only read, never written.
pub fn codex_pets() -> PathBuf {
    crate::connection::codex_home().join("pets")
}

// Downloaded pets: <her folder>/pets, then the project's pets/ (a build
// from source; not a release asset), then Codex's.
pub fn pet_dirs(dir: &Path) -> Vec<PathBuf> {
    [Some(dir.join("pets")), in_tree(1, "pets"), Some(codex_pets())].into_iter().flatten().collect()
}

// The pets, as last found, until one of their folders changes (a pet added
// or taken away changes its folder's time), or she puts one in herself
// (forget_pets). Asked for on every redraw: the folders were read through
// each time.
static PETS: std::sync::Mutex<Option<(u64, Vec<Option<std::time::SystemTime>>, Vec<PetInfo>)>> = std::sync::Mutex::new(None);
static PETS_PUT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

// A pet put in by her (fetch.rs put_pet): what was found is looked for anew.
// The folder's time alone may not say so: it moves on by the clock's tick
// (some 15 ms), and two changes within one look the same. Counted, so a look
// under way as she puts it in is not kept either.
pub fn forget_pets() {
    PETS_PUT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
}

pub fn pets(dir: &Path) -> Vec<PetInfo> {
    let put = PETS_PUT.load(std::sync::atomic::Ordering::SeqCst);
    let stamps: Vec<Option<std::time::SystemTime>> = pet_dirs(dir).iter().map(|root| fs::metadata(root).and_then(|m| m.modified()).ok()).collect();
    let mut kept = PETS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((was, at, found)) = kept.as_ref() {
        if *was == put && *at == stamps {
            return found.clone();
        }
    }
    let found = scan_pets(dir);
    *kept = Some((put, stamps, found.clone()));
    found
}

fn scan_pets(dir: &Path) -> Vec<PetInfo> {
    let mut found: Vec<PetInfo> = Vec::new();
    let codex = codex_pets();
    for root in pet_dirs(dir) {
        let Ok(entries) = fs::read_dir(&root) else { continue };
        let mut ids: Vec<String> = entries
            .flatten()
            .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
            .map(|e| e.file_name().to_string_lossy().into_owned())
            // Not one being put in place (fetch.rs put_pet: .<id>.new).
            .filter(|id| !id.starts_with('.'))
            .collect();
        ids.sort();
        for id in ids {
            let sheet = root.join(&id).join("spritesheet.webp");
            if found.iter().any(|p| p.id == id) || !sheet.exists() {
                continue;
            }
            let meta: Value = fs::read_to_string(root.join(&id).join("pet.json"))
                .ok()
                .and_then(|t| serde_json::from_str(&t).ok())
                .unwrap_or(Value::Null);
            let stamp = fs::metadata(&sheet).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_millis() as u64);
            found.push(PetInfo {
                name: meta.get("displayName").and_then(Value::as_str).unwrap_or(&id).to_string(),
                author: meta.get("author").and_then(Value::as_str).unwrap_or("").to_string(),
                version: if meta.get("spriteVersionNumber").and_then(Value::as_u64) == Some(2) { 2 } else { 1 },
                id,
                sheet,
                from_codex: root == codex,
                stamp,
            });
        }
    }
    found
}

// The URL the page loads a sheet from: Tauri's asset protocol, as
// convertFileSrc makes it (encodeURIComponent of the path).
pub fn asset_url(path: &Path) -> String {
    let mut out = String::new();
    for byte in path.to_string_lossy().bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'!' | b'~' | b'*' | b'\'' | b'(' | b')' => out.push(byte as char),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    if cfg!(any(windows, target_os = "android")) {
        format!("http://asset.localhost/{out}")
    } else {
        format!("asset://localhost/{out}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(v: Value) -> Map<String, Value> {
        let Value::Object(map) = v else { unreachable!() };
        map
    }

    #[test]
    fn the_three_old_widgets_become_the_monitor() {
        // CPU on, the network and the battery off: the monitor on with CPU and memory.
        let mut s = settings(json!({ "widgetsOff": ["net", "battery", "x"], "widgetOrder": ["today", "sys", "net"] }));
        adopt_monitor(&mut s);
        assert_eq!(s["monitor"], json!({ "cpu": true, "mem": true, "net": false, "battery": false, "every": 2 }));
        assert_eq!((s["widgetsOff"].clone(), s["widgetOrder"].clone()), (json!(["x"]), json!(["today", "monitor"])));
        // All three off: the monitor off, with every part ready for when it is on.
        let mut s = settings(json!({ "widgetsOff": ["sys", "net", "battery"] }));
        adopt_monitor(&mut s);
        assert_eq!((s["monitor"]["net"].clone(), s["widgetsOff"].clone()), (json!(true), json!(["monitor"])));
        // Already the monitor's: left as it is.
        let mut s = settings(json!({ "widgetsOff": ["monitor"] }));
        adopt_monitor(&mut s);
        assert!(!s.contains_key("monitor"));
    }

    #[test]
    fn a_config_from_before_keeps_its_look() {
        // Written before 2026-10-13, nothing chosen: Mica, and her island grey.
        let mut s = settings(json!({ "pet": "x" }));
        keep_old_look(&mut s);
        assert_eq!(s["taskbarMaterial"], "mica");
        assert_eq!(s["islandLook"]["dark"]["bare"], false);
        assert_eq!(s["lookDefaults"], 2);
        // What was chosen stays; a light look stays; the dark one as it was.
        let mut s = settings(json!({ "taskbarMaterial": "black", "islandLook": { "light": { "bare": true }, "dark": { "bg": "#000000", "bare": true } } }));
        keep_old_look(&mut s);
        assert_eq!(s["taskbarMaterial"], "black");
        assert_eq!(s["islandLook"]["light"]["bare"], true);
        assert_eq!(s["islandLook"]["dark"]["bg"], "#000000");
        // Reset to the default before (null): the old default, grey.
        let mut s = settings(json!({ "islandLook": { "dark": null } }));
        keep_old_look(&mut s);
        assert_eq!(s["islandLook"]["dark"]["bare"], false);
        // Once only: one written since keeps whatever it says, the new defaults included.
        let mut s = settings(json!({ "lookDefaults": 2, "islandLook": null }));
        keep_old_look(&mut s);
        assert_eq!(s["islandLook"], Value::Null);
        assert!(!s.contains_key("taskbarMaterial"));
        // A new user: the new defaults.
        assert_eq!(defaults()["taskbarMaterial"], "clear");
    }

    #[test]
    fn opening_by_itself_turned_off_is_the_dot_alone_and_the_rest_ring() {
        // Turned off: nothing but the dot; the old key gone.
        let mut s = settings(json!({ "openOnEnd": false, "hold": 30 }));
        adopt_end_cue(&mut s);
        assert_eq!(s, settings(json!({ "endCue": "none", "hold": 30 })));
        // Left on: the new default (none written, the defaults' ring).
        let mut s = settings(json!({ "openOnEnd": true }));
        adopt_end_cue(&mut s);
        assert!(s.is_empty());
        assert_eq!(defaults()["endCue"], "ring");
        assert!(!defaults().contains_key("openOnEnd"));
        // Chosen since: kept, whatever the old key still says.
        let mut s = settings(json!({ "openOnEnd": false, "endCue": "open" }));
        adopt_end_cue(&mut s);
        assert_eq!(s, settings(json!({ "endCue": "open" })));
        // Never had it: nothing to do.
        let mut s = settings(json!({ "endCue": "none" }));
        adopt_end_cue(&mut s);
        assert_eq!(s, settings(json!({ "endCue": "none" })));
    }

    #[test]
    fn quit_by_choice_keeps_her_from_the_hooks_until_she_starts() {
        let dir = std::env::temp_dir().join(format!("wakuwaku-quit-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        // Crashed (nothing marked): a session's hook starts her.
        assert_eq!(clear_quit(&dir), None);
        assert!(may_start(&dir));
        // Quit from her menu: it does not, and the mark says how she went.
        note_quit(&dir, Quit::ByChoice, true, "her menu's Quit");
        assert!(!may_start(&dir));
        assert_eq!(quit_note(&dir).as_deref(), Some("quit by choice (her menu's Quit): a session's hook does not start her until she is opened again"));
        // Started otherwise (by hand, at login, an update): it may again,
        // and what was cleared is told.
        assert!(clear_quit(&dir).is_some_and(|n| n.contains("her menu's Quit")));
        assert!(may_start(&dir));
        assert_eq!(quit_note(&dir), None);
        // POST /quit (by choice unless ?wake=1): marked as the menu marks.
        note_quit(&dir, Quit::ByChoice, true, "POST /quit");
        assert!(!may_start(&dir));
        clear_quit(&dir);
        // POST /quit?wake=1, an update's or a restart's handover: not
        // marked, and a mark left (another copy's) taken away.
        note_quit(&dir, Quit::Wakeable, true, "POST /quit?wake=1");
        assert!(may_start(&dir));
        note_quit(&dir, Quit::ByChoice, true, "POST /quit");
        note_quit(&dir, Quit::Wakeable, true, "an update's handover");
        assert!(may_start(&dir));
        // A quit under way already: the person's choice still marks; one
        // that may be woken coming after it leaves the mark.
        note_quit(&dir, Quit::Wakeable, true, "POST /quit?wake=1");
        note_quit(&dir, Quit::ByChoice, false, "her menu's Quit");
        assert!(!may_start(&dir));
        note_quit(&dir, Quit::Wakeable, false, "POST /quit?wake=1");
        assert!(!may_start(&dir));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_sessions_tags_leave_a_kept_layout() {
        let w = json!({ "id": "wa1", "type": "sep" });
        let mut s = settings(json!({ "taskbarLayout": { "left": ["her"], "center": ["start", "apps", "tags"], "right": [w, "clock"] } }));
        drop_part(&mut s, "tags");
        assert_eq!(s["taskbarLayout"], json!({ "left": ["her"], "center": ["start", "apps"], "right": [w, "clock"] }));
        // The gear too (#38), wherever it was moved to.
        let mut s = settings(json!({ "taskbarLayout": { "left": ["gear", "her"], "center": ["start", "apps"], "right": ["clock", "gear"] } }));
        drop_part(&mut s, "gear");
        assert_eq!(s["taskbarLayout"], json!({ "left": ["her"], "center": ["start", "apps"], "right": ["clock"] }));
        // None kept: nothing to do.
        let mut s = settings(json!({ "taskbarLayout": null }));
        drop_part(&mut s, "tags");
        assert_eq!(s["taskbarLayout"], Value::Null);
    }
}
