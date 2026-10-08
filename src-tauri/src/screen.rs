// The displays' work areas, in physical pixels, read now and then (a window
// getter waits on the main thread, so not on every move).
use tauri::{AppHandle, Monitor};

#[derive(Clone, Copy, Debug)]
pub struct Area {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub sf: f64,
}

#[derive(Clone, Default)]
pub struct Screens {
    pub areas: Vec<Area>,
    pub primary: Option<Area>,
}

fn area(m: &Monitor) -> Area {
    let wa = m.work_area();
    Area { x: wa.position.x, y: wa.position.y, w: wa.size.width as i32, h: wa.size.height as i32, sf: m.scale_factor() }
}

pub fn read(app: &AppHandle) -> Screens {
    Screens {
        areas: app.available_monitors().unwrap_or_default().iter().map(area).collect(),
        primary: app.primary_monitor().ok().flatten().map(|m| area(&m)),
    }
}

impl Screens {
    // The work area holding this point, or the nearest one.
    pub fn near(&self, x: i32, y: i32) -> Option<Area> {
        let gap = |a: &Area| {
            let dx = (a.x - x).max(0).max(x - (a.x + a.w)) as i64;
            let dy = (a.y - y).max(0).max(y - (a.y + a.h)) as i64;
            dx * dx + dy * dy
        };
        self.areas.iter().copied().min_by_key(gap).or(self.primary)
    }

    // A window of this size at (x, y), kept on some display's work area.
    pub fn clamp(&self, (x, y): (i32, i32), (w, h): (i32, i32)) -> (i32, i32) {
        let Some(a) = self.near(x + w / 2, y + h / 2) else { return (x, y) };
        (x.max(a.x).min(a.x + a.w - w), y.max(a.y).min(a.y + a.h - h))
    }
}
