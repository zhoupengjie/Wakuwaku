// A widget dragged from her home over the desktop.
use super::*;

// The widget picker in her home's window (picker.ts, the KDE-like design of
// 2026-10-17): a card, a widget off the strip or a combo dragged over the
// desktop. Her home's page says where the pointer is, in its own px
// (desk_drag_preview, as it moves; desk_drop, let go of there): that is the
// screen's (her home's window's corner and scale, island/ origin), the
// display under it, and that display's desktop page's px; told to that
// page alone (desk:drag-preview, desk:drop: desk-drop.ts shows where it
// would land and puts it there), as it came but for x and y (item and size,
// or a combo's set; the drop's ticket). A page it has left is told null.
// What a page found goes back to her home's page (desk_drag_answer →
// island:desk-answer). A display with no desktop window yet (nothing on it)
// has one made as the drag reaches it, told once its page is up, and kept
// while the drag lasts; then it stays if something was put there (the
// layout says so) and goes as ever if not.
//
// While the drag is over the desktop, its windows are raised over the
// others, right under her home's, and clicks still go through them (her
// home's window holds the pointer for the drag): the desktop's widgets
// and where it lands are seen over whatever window covers them (the user's
// call of 2026-10-17; KDE minimizes the windows, this does not). Put back
// right over the desktop as the drop is answered, as the pointer leaves the
// desktop, or should the buttons be let go of with no word (a page gone).
pub(super) struct Drag {
    // Displays (keys) whose window is kept for the drag, nothing on them.
    pub(super) keep: Vec<String>,
    // The desktop window the pointer was last over (told null once it leaves).
    pub(super) over: Option<String>,
    // What a display's page is to be told once it is up (a window made for
    // the drag, or not ready yet): the last preview, a drop. By the display's key.
    pub(super) preview: Option<(String, Value)>,
    pub(super) drop: Option<(String, Value)>,
    // Counted at each word of the drag: a release of the windows kept is
    // for the turn it was asked at only.
    pub(super) turn: u64,
}
pub(super) static DRAG: Lock<Drag> = Lock::new("desk::drag::DRAG", Drag { keep: Vec::new(), over: None, preview: None, drop: None, turn: 0 });
pub(super) static RAISED: AtomicBool = AtomicBool::new(false);

// Where a point of her home's page is: the display under it (its key), and
// the point in that display's desktop page's px. None: off every display.
pub(super) fn drag_target(sh: &Shared, x: f64, y: f64) -> Option<(String, (f64, f64))> {
    let (pos, sf) = crate::island::origin(sh);
    let p = (pos.0 + (x * sf).round() as i32, pos.1 + (y * sf).round() as i32);
    let area = sh.screens().areas.into_iter().find(|a| contains(a.mon, p))?;
    let key = key_of(sh, area.mon);
    // A window there (made at another scale until the displays settle): its own.
    let (mon, dsf) = desks().iter().find(|d| d.key == key).map_or((area.mon, area.sf), |d| (d.mon, d.sf));
    let at = ((p.0 - mon.0) as f64 / dsf, (p.1 - mon.1) as f64 / dsf);
    Some((key, at))
}

// A preview (drop: false) or a drop of her home's, to the display under it.
pub(super) fn drag_to(sh: &Arc<Shared>, mut payload: Value, drop: bool) {
    let (Some(x), Some(y)) = (payload["x"].as_f64(), payload["y"].as_f64()) else { return };
    let target = if cfg!(windows) { drag_target(sh, x, y) } else { None };
    let Some((key, (dx, dy))) = target else {
        // Off every display (or no desktop windows here): as off the desktop.
        if drop {
            answer_none(sh, &payload);
        }
        return drag_off(sh, drop);
    };
    payload["x"] = json!(dx);
    payload["y"] = json!(dy);
    let event = if drop { "desk:drop" } else { "desk:drag-preview" };
    let here = desks().iter().find(|d| d.key == key).map(|d| (d.label.clone(), d.ready));
    let (left, make) = {
        let mut drag = DRAG.lock().unwrap();
        drag.turn += 1;
        let make = here.is_none() && !drag.keep.contains(&key);
        if here.is_none() && make {
            drag.keep.push(key.clone());
        }
        let now = here.as_ref().map(|(label, _)| label.clone());
        let left = if drag.over != now { std::mem::replace(&mut drag.over, now) } else { None };
        // Its page not up yet: told once it is (desk_ready).
        if !here.as_ref().is_some_and(|(_, ready)| *ready) {
            if drop {
                drag.drop = Some((key.clone(), payload.clone()));
            } else {
                drag.preview = Some((key.clone(), payload.clone()));
            }
        } else if !drop {
            drag.preview = Some((key.clone(), payload.clone()));
        }
        (left, make)
    };
    if let Some(label) = left {
        emit(sh, &label, "desk:drag-preview", Value::Null);
    }
    if let Some((label, true)) = &here {
        emit(sh, label, event, payload);
    }
    if make {
        sh.log(&format!("desk: a widget dragged over a display with nothing on it ({key}): its window made for it"));
        let sh = sh.clone();
        std::thread::spawn(move || sync(&sh));
    }
    if !drop {
        raise(sh);
    }
}

// Off the desktop again (a preview of null), or the drag over (dropped:
// the drop told, its answer in or not coming): every page's preview cleared
// but a dropped one's (it clears its own), the windows back right over the
// desktop, and those kept for the drag let go a moment later.
pub(super) fn drag_off(sh: &Arc<Shared>, dropped: bool) {
    let turn = {
        let mut drag = DRAG.lock().unwrap();
        drag.over = None;
        drag.preview = None;
        drag.turn
    };
    if !dropped {
        let labels: Vec<String> = desks().iter().filter(|d| d.ready).map(|d| d.label.clone()).collect();
        for label in labels {
            emit(sh, &label, "desk:drag-preview", Value::Null);
        }
    }
    lower();
    // Let go once the page's drop is kept (desk_keep, then the layout's
    // sync): a window that has something now stays, the rest go.
    let sh = sh.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(1500));
        let released = {
            let mut drag = DRAG.lock().unwrap();
            if drag.turn != turn || drag.keep.is_empty() {
                return;
            }
            drag.drop = None;
            std::mem::take(&mut drag.keep)
        };
        sh.log(&format!("desk: the drag over, the windows made for it let go of unless kept ({})", released.join(", ")));
        sync(&sh);
    });
}

// A drop no page can take: said so at once, for her home's page.
pub(super) fn answer_none(sh: &Shared, payload: &Value) {
    let _ = sh.app.emit_to("island", "island:desk-answer", json!({ "ticket": payload["ticket"], "ok": false, "where": "desk", "why": "none" }));
}

// The desktop's windows over the others, right under her home's.
pub(super) fn raise(sh: &Arc<Shared>) {
    if RAISED.swap(true, Ordering::SeqCst) {
        return;
    }
    sh.log("desk: raised over the windows for a widget dragged from her home");
    imp::keep_all_low();
    // Put back should the drag end with no word (the buttons let go of,
    // her home's page gone): looked at every half second while raised.
    let sh = sh.clone();
    std::thread::spawn(move || {
        let mut up_for = 0;
        while RAISED.load(Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_millis(500));
            up_for = if crate::pet::any_down().unwrap_or(false) { 0 } else { up_for + 1 };
            if up_for >= 3 && RAISED.load(Ordering::SeqCst) {
                sh.log("desk: no button held for a while, the drag over: put back");
                drag_off(&sh, false);
            }
        }
    });
}

// Right over the desktop again.
pub(super) fn lower() {
    if RAISED.swap(false, Ordering::SeqCst) {
        imp::keep_all_low();
        if let Some(sh) = SHARED.get() {
            sh.log("desk: put back right over the desktop");
        }
    }
}

// Her home's page: where the pointer is over the desktop ({ x, y, item,
// size } or { x, y, set }, its px), or null (off it again).
#[tauri::command]
pub async fn desk_drag_preview(app: tauri::AppHandle, window: WebviewWindow, at: Option<Value>) {
    count(&COMMANDS);
    if window.label() != "island" {
        return;
    }
    let sh = shared_of(&app);
    match at {
        Some(at) if at.is_object() => drag_to(&sh, at, false),
        _ => drag_off(&sh, false),
    }
}

// Her home's page: let go of over the desktop ({ x, y, item, size, ticket }
// or { x, y, set, ticket }).
#[tauri::command]
pub async fn desk_drop(app: tauri::AppHandle, window: WebviewWindow, at: Value) {
    count(&COMMANDS);
    if window.label() != "island" || !at.is_object() {
        return;
    }
    drag_to(&shared_of(&app), at, true);
}

// A desktop page's word on what is dragged over it ({ ticket, ok, where,
// why, name }; ticket null: the preview's), to her home's page. A drop's:
// the drag is over.
#[tauri::command]
pub async fn desk_drag_answer(app: tauri::AppHandle, window: WebviewWindow, said: Value) {
    count(&COMMANDS);
    if !window.label().starts_with("desk-") {
        return;
    }
    let dropped = said.get("ticket").is_some_and(|t| !t.is_null());
    let _ = app.emit_to("island", "island:desk-answer", said);
    if dropped {
        drag_off(&shared_of(&app), true);
    }
}

// A page just up: what the drag has for it (its window made for it).
pub(super) fn drag_catch_up(sh: &Shared, label: &str, key: &str) {
    let (preview, drop) = {
        let mut drag = DRAG.lock().unwrap();
        let preview = drag.preview.as_ref().filter(|(k, _)| k == key).map(|(_, p)| p.clone());
        let drop = drag.drop.take_if(|(k, _)| k == key).map(|(_, p)| p);
        if preview.is_some() || drop.is_some() {
            drag.over = Some(label.to_string());
        }
        (preview, drop)
    };
    if let Some(p) = preview {
        emit(sh, label, "desk:drag-preview", p);
    }
    if let Some(p) = drop {
        emit(sh, label, "desk:drop", p);
    }
}

