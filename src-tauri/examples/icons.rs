// The desktop's icons as her strip sees them (src/icons.rs), to look and try:
//
//   icons list                  the desktop: arranges itself or not, its grid,
//                               its windows, the work area, every icon's place
//   icons save <file>           the same, into a file
//   icons tell <which>          the desktop told the work area changed, by one
//                               window (top: Progman or its WorkerW; defview;
//                               list), then listed again
//   icons put <file>            every icon in the file put back at its place
//   icons work <l> <t> <r> <b>  the work area set quietly, as her taskbar does
//   icons auto on|off           the desktop arranging its icons itself, or not
//   icons snap on|off           keeping them to its grid, or not
//   icons take <edge> <x> <y> <w> <h> <dir>
//                               as her strip taken along an edge (top, left,
//                               right): the work area set quietly to the
//                               display less it, then icons::strip_taken with
//                               <dir> as her data folder
//   icons hold <dir>            icons::hold: where they are, before giving back
//   icons back <l> <t> <r> <b> <dir>
//                               as given back: the work area set quietly,
//                               then icons::given_back
// Its locks are lock.rs Locks.
#[path = "../src/lock.rs"]
#[allow(dead_code)]
mod lock;
#[path = "../src/icons.rs"]
mod icons;

#[cfg(windows)]
fn main() {
    #[link(name = "user32")]
    extern "system" {
        fn SetProcessDpiAwarenessContext(context: isize) -> i32;
        fn SystemParametersInfoW(action: u32, param: u32, pv: *mut std::ffi::c_void, flags: u32) -> i32;
    }
    // The screen's own px, as hers are.
    // SAFETY: a plain call, before any window.
    unsafe { SetProcessDpiAwarenessContext(-4) };
    let args: Vec<String> = std::env::args().collect();
    let out = |v: &serde_json::Value| println!("{}", serde_json::to_string_pretty(v).unwrap());
    match args.get(1).map(String::as_str) {
        Some("save") => {
            let file = args.get(2).expect("a file");
            std::fs::write(file, serde_json::to_string_pretty(&icons::describe()).unwrap()).unwrap();
            println!("saved");
        }
        Some("tell") => {
            let told = icons::with_com(|| {
                let d = icons::desktop().expect("the desktop");
                let to = match args.get(2).map(String::as_str) {
                    Some("defview") => d.defview,
                    Some("list") => d.list,
                    _ => d.top,
                };
                d.tell(to)
            });
            println!("told: {told}");
            std::thread::sleep(std::time::Duration::from_millis(700));
            out(&icons::describe());
        }
        Some("put") => {
            let file = args.get(2).expect("a file");
            let saved: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap();
            let done = icons::with_com(|| {
                let d = icons::desktop().expect("the desktop");
                let items = d.items();
                let moves: Vec<(&icons::Item, icons::Point)> = saved["items"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter_map(|s| {
                        let item = items.iter().find(|i| i.name == s["name"].as_str().unwrap_or(""))?;
                        let to = icons::Point { x: s["x"].as_i64()? as i32, y: s["y"].as_i64()? as i32 };
                        (item.at != to).then_some((item, to))
                    })
                    .collect();
                let n = moves.len();
                (d.place(&moves), n)
            });
            println!("put back: {done:?}");
            std::thread::sleep(std::time::Duration::from_millis(500));
            out(&icons::describe());
        }
        Some(which @ ("auto" | "snap")) => {
            let on = args.get(2).map(String::as_str) == Some("on");
            let flag = if which == "auto" { 1 } else { 4 };
            let ok = icons::with_com(|| icons::desktop().expect("the desktop").set_flag(flag, on));
            println!("{which} {on}: {ok}");
            std::thread::sleep(std::time::Duration::from_millis(500));
            out(&icons::describe());
        }
        Some("work") => {
            let n: Vec<i32> = args[2..6].iter().map(|a| a.parse().unwrap()).collect();
            let mut r = icons::Rect { left: n[0], top: n[1], right: n[2], bottom: n[3] };
            // SAFETY: our own struct; set without telling any window.
            let ok = unsafe { SystemParametersInfoW(0x2F, 0, &mut r as *mut icons::Rect as *mut std::ffi::c_void, 0) };
            println!("work area set: {ok}");
            out(&icons::describe());
        }
        Some("take") => {
            let edge = args[2].as_str();
            let n: Vec<i32> = args[3..7].iter().map(|a| a.parse().unwrap()).collect();
            let strip = (n[0], n[1], n[2], n[3]);
            // From the work area as it is (a strip of hers along the bottom keeps its own).
            let mut r = icons::Rect::default();
            // SAFETY: our own struct, of the size the call writes.
            unsafe { SystemParametersInfoW(0x30, 0, &mut r as *mut icons::Rect as *mut std::ffi::c_void, 0) };
            // The display less the strip, as taskbar/win/work.rs strip_moved; the bottom as it is.
            let mut r = icons::Rect { left: 0, top: 0, right: 2560, bottom: r.bottom };
            match edge {
                "left" => r.left = strip.0 + strip.2,
                "right" => r.right = strip.0,
                "top" => r.top = strip.1 + strip.3,
                _ => r.bottom = strip.1,
            }
            // SAFETY: our own struct; set without telling any window.
            unsafe { SystemParametersInfoW(0x2F, 0, &mut r as *mut icons::Rect as *mut std::ffi::c_void, 0) };
            icons::strip_taken(std::path::Path::new(&args[7]), strip, edge, |line| println!("{line}"));
            // Its looks 3 s and 8 s after.
            std::thread::sleep(std::time::Duration::from_secs(9));
            out(&icons::describe());
        }
        Some("hold") => {
            for line in icons::hold(std::path::Path::new(&args[2])) {
                println!("{line}");
            }
            out(&icons::describe());
        }
        Some("back") => {
            let n: Vec<i32> = args[2..6].iter().map(|a| a.parse().unwrap()).collect();
            let mut r = icons::Rect { left: n[0], top: n[1], right: n[2], bottom: n[3] };
            // SAFETY: our own struct; set without telling any window.
            unsafe { SystemParametersInfoW(0x2F, 0, &mut r as *mut icons::Rect as *mut std::ffi::c_void, 0) };
            for line in icons::given_back(std::path::Path::new(&args[6])) {
                println!("{line}");
            }
            out(&icons::describe());
        }
        _ => out(&icons::describe()),
    }
}

#[cfg(not(windows))]
fn main() {}
