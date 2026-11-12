// The input method in front as her taskbar hears it from Windows' input
// switcher (src/inputs.rs), to look and try:
//
//   inputs list          the input methods to pick from (Windows' language
//                        list), each with its id, name and icon
//   inputs watch <s>     the input method in front each time it changes,
//                        for s seconds (10 if not said)
//   inputs pick <id>     the window in front switched to one of them (an id
//                        as the list prints it), then watched for 3 s
//
// Icons are not drawn here: each says the size it was taken at.
// Its locks are lock.rs Locks.
#[path = "../src/lock.rs"]
#[allow(dead_code)]
mod lock;
#[path = "../src/inputs.rs"]
mod inputs;

fn draw(icon: isize, size: i32) -> Option<String> {
    (icon != 0).then(|| format!("<icon at {size} px>"))
}

#[cfg(windows)]
#[link(name = "user32")]
extern "system" {
    fn GetForegroundWindow() -> *mut std::ffi::c_void;
    fn GetWindowThreadProcessId(hwnd: *mut std::ffi::c_void, pid: *mut u32) -> u32;
    fn GetKeyboardLayout(thread: u32) -> isize;
}

// As the taskbar's keys thread does: the layout in front told four times a second.
#[cfg(windows)]
fn watch(seconds: u64) {
    inputs::set_size(24);
    inputs::start(draw, Box::new(|| println!("now {}", inputs::now())), Box::new(|line: &str| println!("log {line}")));
    let end = std::time::Instant::now() + std::time::Duration::from_secs(seconds);
    while std::time::Instant::now() < end {
        // SAFETY: plain queries about the window in front.
        inputs::layout(unsafe { GetKeyboardLayout(GetWindowThreadProcessId(GetForegroundWindow(), std::ptr::null_mut())) });
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
    for (key, png) in inputs::icons(true) {
        println!("icon {key}: {png}");
    }
    if let Some(thread) = inputs::stop() {
        let _ = thread.join();
    }
}

#[cfg(windows)]
fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("list") => {
            inputs::start(draw, Box::new(|| {}), Box::new(|line: &str| println!("log {line}")));
            std::thread::sleep(std::time::Duration::from_millis(500));
            println!("{}", serde_json::to_string_pretty(&inputs::list()).unwrap());
            if let Some(thread) = inputs::stop() {
                let _ = thread.join();
            }
        }
        Some("watch") => watch(args.get(2).and_then(|s| s.parse().ok()).unwrap_or(10)),
        Some("pick") => {
            let id = args.get(2).expect("an id");
            inputs::start(draw, Box::new(|| println!("now {}", inputs::now())), Box::new(|line: &str| println!("log {line}")));
            std::thread::sleep(std::time::Duration::from_millis(500));
            println!("picked: {}", inputs::pick(id));
            std::thread::sleep(std::time::Duration::from_secs(3));
            if let Some(thread) = inputs::stop() {
                let _ = thread.join();
            }
        }
        _ => eprintln!("inputs list | watch [seconds] | pick <id>"),
    }
}

// Windows' input switcher only: elsewhere there is nothing to look at.
#[cfg(not(windows))]
fn main() {}
