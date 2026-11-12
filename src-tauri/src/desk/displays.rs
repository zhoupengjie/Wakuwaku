// Each display's picture, and each display's own name (its key).
use super::*;

// Read for each window, told when it changed: the file (by the asset
// protocol), how it is laid, the colour round it, the display and all of
// them from the window's corner (the window is the display), its scale.
pub fn read_papers(sh: &Shared) {
    let desks: Vec<(String, Mon, f64)> = desks().iter().filter(|d| d.ready).map(|d| (d.label.clone(), d.mon, d.sf)).collect();
    if desks.is_empty() {
        return;
    }
    let areas = sh.screens().areas;
    let left = areas.iter().map(|a| a.mon.0).min().unwrap_or(0);
    let top = areas.iter().map(|a| a.mon.1).min().unwrap_or(0);
    let right = areas.iter().map(|a| a.mon.0 + a.mon.2).max().unwrap_or(0);
    let bottom = areas.iter().map(|a| a.mon.1 + a.mon.3).max().unwrap_or(0);
    for (label, mon, sf) in desks {
        let Some(paper) = crate::wallpaper::read(mon) else { continue };
        if let Some(file) = &paper.file {
            let _ = sh.app.asset_protocol_scope().allow_file(file);
        }
        let rel = |(x, y, w, h): Mon| json!([(x - mon.0) as f64 / sf, (y - mon.1) as f64 / sf, w as f64 / sf, h as f64 / sf]);
        let url = paper.file.as_ref().map(|f| format!("{}?v={}", crate::data::asset_url(f), paper.stamp));
        let now = json!({ "url": url, "position": paper.position, "color": paper.color, "mon": rel(mon), "screen": rel((left, top, right - left, bottom - top)), "sf": sf });
        // Not for a window gone meanwhile.
        let here = super::desks().iter().any(|d| d.label == label);
        let changed = here
            && told(&label, |t| {
                let changed = t.paper != now;
                if changed {
                    t.paper = now.clone();
                }
                changed
            });
        if changed {
            sh.log(&format!("desk: {label} wallpaper {:?}, {}, {}", paper.file, paper.position, paper.color));
            emit(sh, &label, "desk:paper", now);
        }
    }
}

// Windows said its colours, the picture or the displays changed, or the
// half minute is up: looked at again.
pub(super) fn look_again() {
    let Some(sh) = SHARED.get() else { return };
    let look = look_read();
    let was = LOOK.lock().unwrap().replace(look.clone());
    if was != Some(look) {
        refresh(sh);
    }
    read_papers(sh);
}

// --- A display's own name ---------------------------------------------------------------------
// The monitor's device interface (the same panel on the same port, whatever
// number Windows gives the display this time), shortened: "d" and 12 hex
// digits. Without it, its device name (\\.\DISPLAY1), and without that its
// size and place.
pub(super) static KEYS: Lock<Vec<(Mon, String)>> = Lock::new("desk::displays::KEYS", Vec::new());
pub fn key_of(sh: &Shared, mon: Mon) -> String {
    if let Some((_, k)) = KEYS.lock().unwrap().iter().find(|(m, _)| *m == mon) {
        return k.clone();
    }
    let name = crate::main_wait::monitors(&sh.app).into_iter().find(|m| (m.position().x, m.position().y, m.size().width as i32, m.size().height as i32) == mon).and_then(|m| m.name().cloned());
    let key = key_for(name.as_deref(), mon);
    let mut keys = KEYS.lock().unwrap();
    keys.retain(|(m, _)| *m != mon);
    keys.push((mon, key.clone()));
    key
}

// The key from a display's device name and its rect (screen.rs read keeps
// one for each display; the setting homeDisplay holds one, #104).
pub fn key_for(name: Option<&str>, mon: Mon) -> String {
    let id = name.and_then(imp::device_of).or_else(|| name.map(str::to_string)).unwrap_or_else(|| format!("{}x{}@{},{}", mon.2, mon.3, mon.0, mon.1));
    short_key(&id)
}

// FNV-1a over the name, 48 bits of it.
pub fn short_key(id: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in id.to_ascii_lowercase().bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("d{:012x}", h & 0xFFFF_FFFF_FFFF)
}
