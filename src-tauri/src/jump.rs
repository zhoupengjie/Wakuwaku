// To a session's window: the terminal, the editor or the app it runs in,
// brought to the front when you click the session. Windows only.
//
// Each session remembers its process and the processes above it (its
// chain), each with when it started, so a number Windows has handed to
// another process since is not mistaken for it:
//   Claude Code   the process on the other end of the hook's connection
//                 (server.rs), found in the TCP table
//   Codex         the hook command's own parents, from Codex up, past the
//                 shell it runs hooks in (main.rs codex_hook)
// Going there, while the session's own process (the chain's first) still
// runs: the first process up the chain, still the same one, that has
// a window of its own (or a console window, kept by a conhost under it);
// with several (VS Code, Windows Terminal), the one whose title names the
// session or its project. Else the window its console belongs to: Windows
// Terminal hosts a console started outside it, and owns the console's
// hidden window. A session of the Claude desktop app also has the
// app open it (claude://code/continue?session=local_…), found by its id in
// the app's own records of its sessions.

pub type Chain = Vec<(u32, u64)>;

// Where Codex's hook command puts its chain in the event it hands on.
pub const CODEX_CHAIN: &str = "wakuwaku_chain";

#[cfg(windows)]
mod imp {
    use std::collections::HashMap;
    use std::ffi::c_void;
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;

    use super::Chain;

    type Hwnd = *mut c_void;
    // As main.rs declares them too (one signature per function).
    type Handle = isize;

    // Where a chain stops: the processes every program runs under.
    const ROOTS: [&str; 10] = [
        "explorer.exe",
        "sihost.exe",
        "svchost.exe",
        "services.exe",
        "wininit.exe",
        "winlogon.exe",
        "csrss.exe",
        "smss.exe",
        "userinit.exe",
        "system",
    ];
    // What Codex runs its hook commands in.
    const SHELLS: [&str; 6] = ["powershell.exe", "pwsh.exe", "cmd.exe", "bash.exe", "sh.exe", "conhost.exe"];
    // What keeps a console window for the program under it.
    const CONSOLES: [&str; 2] = ["conhost.exe", "openconsole.exe"];
    const MAX_CHAIN: usize = 16;

    #[repr(C)]
    struct ProcessEntry {
        size: u32,
        usage: u32,
        pid: u32,
        heap: usize,
        module: u32,
        threads: u32,
        parent: u32,
        priority: i32,
        flags: u32,
        exe: [u16; 260],
    }

    #[repr(C)]
    #[derive(Default)]
    struct FileTime {
        low: u32,
        high: u32,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn CreateToolhelp32Snapshot(flags: u32, pid: u32) -> Handle;
        fn Process32FirstW(snap: Handle, entry: *mut ProcessEntry) -> i32;
        fn Process32NextW(snap: Handle, entry: *mut ProcessEntry) -> i32;
        fn CloseHandle(handle: Handle) -> i32;
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> Handle;
        fn GetProcessTimes(process: Handle, created: *mut FileTime, exited: *mut FileTime, kernel: *mut FileTime, user: *mut FileTime) -> i32;
        fn GetCurrentThreadId() -> u32;
        fn AttachConsole(pid: u32) -> i32;
        fn FreeConsole() -> i32;
        fn GetConsoleWindow() -> Hwnd;
    }

    #[link(name = "iphlpapi")]
    extern "system" {
        fn GetExtendedTcpTable(table: *mut c_void, size: *mut u32, order: i32, family: u32, class: u32, reserved: u32) -> u32;
    }

    #[link(name = "user32")]
    extern "system" {
        fn EnumWindows(each: extern "system" fn(Hwnd, isize) -> i32, param: isize) -> i32;
        fn IsWindowVisible(hwnd: Hwnd) -> i32;
        fn IsIconic(hwnd: Hwnd) -> i32;
        fn GetWindow(hwnd: Hwnd, cmd: u32) -> Hwnd;
        fn GetWindowTextW(hwnd: Hwnd, text: *mut u16, max: i32) -> i32;
        fn GetWindowLongW(hwnd: Hwnd, index: i32) -> i32;
        fn GetWindowThreadProcessId(hwnd: Hwnd, pid: *mut u32) -> u32;
        fn ShowWindow(hwnd: Hwnd, cmd: i32) -> i32;
        fn SetForegroundWindow(hwnd: Hwnd) -> i32;
        fn GetForegroundWindow() -> Hwnd;
        fn BringWindowToTop(hwnd: Hwnd) -> i32;
        fn AttachThreadInput(from: u32, to: u32, attach: i32) -> i32;
        fn GetAncestor(hwnd: Hwnd, flags: u32) -> Hwnd;
    }

    #[link(name = "shell32")]
    extern "system" {
        fn ShellExecuteW(hwnd: Hwnd, op: *const u16, file: *const u16, params: *const u16, dir: *const u16, show: i32) -> isize;
    }

    const TH32CS_SNAPPROCESS: u32 = 0x2;
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const AF_INET: u32 = 2;
    const TCP_TABLE_OWNER_PID_ALL: u32 = 5;
    const GW_OWNER: u32 = 4;
    const GWL_EXSTYLE: i32 = -20;
    const WS_EX_TOOLWINDOW: u32 = 0x80;
    const SW_RESTORE: i32 = 9;
    const SW_SHOWNORMAL: i32 = 1;
    const GA_ROOTOWNER: u32 = 3;

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    // Every process: its parent and its exe's name, in lower case.
    fn processes() -> HashMap<u32, (u32, String)> {
        let mut out = HashMap::new();
        // SAFETY: a snapshot we close below; the entry is sized as the API asks.
        unsafe {
            let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snap == 0 || snap == -1 {
                return out;
            }
            let mut e: ProcessEntry = std::mem::zeroed();
            e.size = std::mem::size_of::<ProcessEntry>() as u32;
            let mut ok = Process32FirstW(snap, &mut e);
            while ok != 0 {
                let len = e.exe.iter().position(|&c| c == 0).unwrap_or(e.exe.len());
                out.insert(e.pid, (e.parent, String::from_utf16_lossy(&e.exe[..len]).to_lowercase()));
                ok = Process32NextW(snap, &mut e);
            }
            CloseHandle(snap);
        }
        out
    }

    // When a process started (100 ns since 1601), or None when gone or not ours to ask.
    fn started(pid: u32) -> Option<u64> {
        // SAFETY: a handle we close; the times are written into our own structs.
        unsafe {
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if process == 0 {
                return None;
            }
            let (mut created, mut exited, mut kernel, mut user) = (FileTime::default(), FileTime::default(), FileTime::default(), FileTime::default());
            let ok = GetProcessTimes(process, &mut created, &mut exited, &mut kernel, &mut user);
            CloseHandle(process);
            (ok != 0).then(|| (u64::from(created.high) << 32) | u64::from(created.low))
        }
    }

    // A process and the ones above it, up to what every program runs under.
    // A parent never starts after its child: one that did has a reused number.
    pub fn chain_of(pid: u32) -> Chain {
        named_chain(pid, &processes()).into_iter().map(|(pid, at, _)| (pid, at)).collect()
    }

    // The chain of a Codex session, from inside its hook command: from Codex
    // up, past this command and the shell Codex runs it in.
    pub fn hook_chain() -> Chain {
        named_chain(std::process::id(), &processes())
            .into_iter()
            .skip(1)
            .skip_while(|(_, _, name)| SHELLS.contains(&name.as_str()))
            .map(|(pid, at, _)| (pid, at))
            .collect()
    }

    fn named_chain(pid: u32, all: &HashMap<u32, (u32, String)>) -> Vec<(u32, u64, String)> {
        let mut chain: Vec<(u32, u64, String)> = Vec::new();
        let mut cur = pid;
        while chain.len() < MAX_CHAIN {
            let Some((parent, name)) = all.get(&cur) else { break };
            if ROOTS.contains(&name.as_str()) {
                break;
            }
            let Some(at) = started(cur) else { break };
            if chain.last().is_some_and(|&(_, child, _)| at > child) {
                break;
            }
            chain.push((cur, at, name.clone()));
            if *parent == 0 || *parent == cur {
                break;
            }
            cur = *parent;
        }
        chain
    }

    // The process on the other end of a connection to this port: the
    // client's own side, which is local too.
    pub fn client_of(client_port: u16, server_port: u16) -> Option<u32> {
        let port = |v: u32| u16::from_be(v as u16);
        let mut size = 0u32;
        // SAFETY: first asking the size, then filling a buffer of that size.
        unsafe {
            GetExtendedTcpTable(std::ptr::null_mut(), &mut size, 0, AF_INET, TCP_TABLE_OWNER_PID_ALL, 0);
            let mut table = vec![0u32; size as usize / 4 + 64];
            size = (table.len() * 4) as u32;
            if GetExtendedTcpTable(table.as_mut_ptr().cast(), &mut size, 0, AF_INET, TCP_TABLE_OWNER_PID_ALL, 0) != 0 {
                return None;
            }
            // { count, then rows of state, local address, local port, remote address, remote port, pid }
            let count = table[0] as usize;
            table[1..].chunks_exact(6).take(count).find(|row| port(row[2]) == client_port && port(row[4]) == server_port).map(|row| row[5])
        }
    }

    struct Window {
        hwnd: isize,
        pid: u32,
        title: String,
    }

    extern "system" fn collect(hwnd: Hwnd, param: isize) -> i32 {
        // SAFETY: param is the Vec windows() passed; the window handle comes from EnumWindows.
        unsafe {
            let list = &mut *(param as *mut Vec<Window>);
            let is_tool = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW != 0;
            if IsWindowVisible(hwnd) == 0 || !GetWindow(hwnd, GW_OWNER).is_null() || is_tool {
                return 1;
            }
            let mut text = [0u16; 512];
            let len = GetWindowTextW(hwnd, text.as_mut_ptr(), text.len() as i32);
            if len <= 0 {
                return 1;
            }
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, &mut pid);
            list.push(Window { hwnd: hwnd as isize, pid, title: String::from_utf16_lossy(&text[..len as usize]) });
        }
        1
    }

    // The visible top-level windows, front to back, with titles; none of ours.
    fn windows() -> Vec<Window> {
        let mut list: Vec<Window> = Vec::new();
        // SAFETY: collect() only runs during this call, while list lives.
        unsafe {
            EnumWindows(collect, &mut list as *mut Vec<Window> as isize);
        }
        let me = std::process::id();
        list.retain(|w| w.pid != me);
        list
    }

    // The window a session runs in: the first process up its chain (still the
    // same one) with a window, or a console window kept for it.
    pub fn window_of(chain: &Chain, hints: &[&str], needs_own: bool) -> Option<isize> {
        // Its own process gone, the session is over: no window is its.
        let &(first, at) = chain.first()?;
        if needs_own && started(first) != Some(at) {
            return None;
        }
        let all = processes();
        let windows = windows();
        for &(pid, at) in chain {
            if started(pid) != Some(at) {
                continue;
            }
            let consoles: Vec<u32> = all.iter().filter(|(_, (parent, name))| *parent == pid && CONSOLES.contains(&name.as_str())).map(|(p, _)| *p).collect();
            let its: Vec<&Window> = windows.iter().filter(|w| w.pid == pid || consoles.contains(&w.pid)).collect();
            if !its.is_empty() {
                let named = hints.iter().filter(|h| !h.is_empty()).find_map(|h| its.iter().find(|w| w.title.contains(h)));
                return named.or(its.first()).map(|w| w.hwnd);
            }
            if let Some(hwnd) = console_window(pid) {
                return Some(hwnd);
            }
        }
        None
    }

    // The window a process's console shows in: the console's own, or the
    // terminal's that hosts it (it owns the console's hidden window). Never
    // while this program has a console of its own (a debug build): it would
    // have to let go of it to look.
    fn console_window(pid: u32) -> Option<isize> {
        static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());
        let _one = ONE_AT_A_TIME.lock().unwrap();
        // SAFETY: attached only for these few calls, and let go of again.
        unsafe {
            if !GetConsoleWindow().is_null() || AttachConsole(pid) == 0 {
                return None;
            }
            let console = GetConsoleWindow();
            FreeConsole();
            if console.is_null() {
                return None;
            }
            if IsWindowVisible(console) != 0 {
                return Some(console as isize);
            }
            let owner = GetAncestor(console, GA_ROOTOWNER);
            (!owner.is_null() && owner != console && IsWindowVisible(owner) != 0).then_some(owner as isize)
        }
    }

    // To the front, restored if minimized. Windows hands the foreground only
    // to the one who has it (or was last clicked); else borrow its input.
    pub fn bring(hwnd: isize) -> bool {
        let h = hwnd as Hwnd;
        // SAFETY: a window handle just found; a stale one only makes the calls fail.
        unsafe {
            if IsIconic(h) != 0 {
                ShowWindow(h, SW_RESTORE);
            }
            // The switch itself may land a moment later: its answer is the one to go by.
            if SetForegroundWindow(h) != 0 {
                return true;
            }
            let front = GetForegroundWindow();
            let theirs = GetWindowThreadProcessId(front, std::ptr::null_mut());
            let mine = GetCurrentThreadId();
            let attached = theirs != 0 && theirs != mine && AttachThreadInput(mine, theirs, 1) != 0;
            BringWindowToTop(h);
            let brought = SetForegroundWindow(h) != 0;
            if attached {
                AttachThreadInput(mine, theirs, 0);
            }
            brought || GetForegroundWindow() == h
        }
    }

    // Where the Claude desktop app keeps its records of sessions: its own
    // folder, or the one Windows gives it as an installed (MSIX) app.
    fn desktop_stores() -> Vec<PathBuf> {
        let mut out = Vec::new();
        if let Some(roaming) = std::env::var_os("APPDATA") {
            out.push(PathBuf::from(roaming).join("Claude").join("claude-code-sessions"));
        }
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            let packages = PathBuf::from(local).join("Packages");
            for entry in std::fs::read_dir(&packages).into_iter().flatten().flatten() {
                if entry.file_name().to_string_lossy().starts_with("Claude_") {
                    out.push(entry.path().join("LocalCache").join("Roaming").join("Claude").join("claude-code-sessions"));
                }
            }
        }
        out
    }

    fn find_desktop(dir: &Path, needle: &str, depth: u32) -> Option<String> {
        for entry in std::fs::read_dir(dir).ok()?.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if path.is_dir() {
                if depth > 0 {
                    if let Some(found) = find_desktop(&path, needle, depth - 1) {
                        return Some(found);
                    }
                }
                continue;
            }
            let Some(id) = name.strip_suffix(".json").filter(|n| n.starts_with("local_")) else { continue };
            // The ids are at the top of the record; the rest can be long.
            let mut head = vec![0u8; 2048];
            let n = std::fs::File::open(&path).and_then(|mut f| std::io::Read::read(&mut f, &mut head)).unwrap_or(0);
            if String::from_utf8_lossy(&head[..n]).contains(needle) && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
                return Some(id.to_string());
            }
        }
        None
    }

    // The desktop app's own id for a Claude Code session it runs (local_…),
    // remembered once found.
    pub fn desktop_session(cli_session: &str) -> Option<String> {
        static KNOWN: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);
        if cli_session.is_empty() {
            return None;
        }
        if let Some(id) = KNOWN.lock().unwrap().as_ref().and_then(|m| m.get(cli_session)) {
            return Some(id.clone());
        }
        let needle = format!("\"cliSessionId\":\"{cli_session}\"");
        let id = desktop_stores().iter().find_map(|dir| find_desktop(dir, &needle, 3))?;
        KNOWN.lock().unwrap().get_or_insert_with(HashMap::new).insert(cli_session.to_string(), id.clone());
        Some(id)
    }

    pub fn open_url(url: &str) -> bool {
        let (op, file) = (wide("open"), wide(url));
        // SAFETY: two NUL-terminated strings that outlive the call.
        unsafe { ShellExecuteW(std::ptr::null_mut(), op.as_ptr(), file.as_ptr(), std::ptr::null(), std::ptr::null(), SW_SHOWNORMAL) > 32 }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::Chain;

    pub fn chain_of(_pid: u32) -> Chain {
        Vec::new()
    }
    pub fn hook_chain() -> Chain {
        Vec::new()
    }
    pub fn client_of(_client_port: u16, _server_port: u16) -> Option<u32> {
        None
    }
    pub fn window_of(_chain: &Chain, _hints: &[&str], _needs_own: bool) -> Option<isize> {
        None
    }
    pub fn bring(_hwnd: isize) -> bool {
        false
    }
    pub fn desktop_session(_cli_session: &str) -> Option<String> {
        None
    }
    pub fn open_url(_url: &str) -> bool {
        false
    }
}

pub use imp::{chain_of, client_of, hook_chain};

// A chain as the hooks' messages carry it: [[pid, started], ...].
pub fn chain_json(chain: &Chain) -> serde_json::Value {
    serde_json::Value::Array(chain.iter().map(|&(pid, at)| serde_json::json!([pid, at])).collect())
}

pub fn chain_from_json(value: Option<&serde_json::Value>) -> Chain {
    value
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|i| Some((u32::try_from(i.get(0)?.as_u64()?).ok()?, i.get(1)?.as_u64()?)))
                .take(16)
                .collect()
        })
        .unwrap_or_default()
}

// To the session: its window to the front; for a session of the Claude
// desktop app, the app opens it too. True when anything was found to go to.
pub fn go(session: &str, chain: &Chain, hints: &[&str]) -> bool {
    // The desktop app's session is the app's to show, its process running or not.
    let desktop = imp::desktop_session(session);
    let window = imp::window_of(chain, hints, desktop.is_none());
    let brought = window.is_some_and(imp::bring);
    let opened = desktop.is_some_and(|id| imp::open_url(&format!("claude://code/continue?session={id}")));
    brought || opened
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chains_travel_as_json() {
        let chain: Chain = vec![(1234, 133_000_000_000_000_000), (88, 133_000_000_000_000_001)];
        assert_eq!(chain_from_json(Some(&chain_json(&chain))), chain);
        assert!(chain_from_json(Some(&serde_json::json!([[1], "x", [-1, 2]]))).is_empty());
        assert!(chain_from_json(None).is_empty());
    }

    #[cfg(windows)]
    #[test]
    fn this_process_has_a_chain() {
        let chain = chain_of(std::process::id());
        assert_eq!(chain.first().map(|c| c.0), Some(std::process::id()));
        assert!(chain.windows(2).all(|w| w[0].1 >= w[1].1), "a parent never starts after its child");
    }

    #[cfg(windows)]
    #[test]
    fn a_connection_names_its_client() {
        let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let server_port = server.local_addr().unwrap().port();
        let client = std::net::TcpStream::connect(("127.0.0.1", server_port)).unwrap();
        let client_port = client.local_addr().unwrap().port();
        assert_eq!(client_of(client_port, server_port), Some(std::process::id()));
    }
}
