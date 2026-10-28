// To a session's window: the terminal, the editor or the app it runs in,
// brought to the front when you click the session. Windows only.
//
// Each session remembers its process and the processes above it (its
// chain), each with when it started, so a number Windows has handed to
// another process since is not mistaken for it:
//   Claude Code   the process on the other end of the hook's connection
//                 (server.rs), found in the TCP table
//   Codex         the hook command's own parents, from Codex up, past the
//                 shell it runs hooks in (main.rs codex_hook); Antigravity's,
//                 ZCode's and Reasonix's the same (Reasonix Studio's: its
//                 kernel, then the app's window process)
//   OpenCode      a program connected to its background service, where her
//                 plugin runs (front_chain): its TUI, or the run
//   DeepSeek H.   as Claude Code's, the hook on the other end of the
//                 connection (its sandbox keeps it from looking above
//                 itself): the outermost process above it, the app
//                 (server.rs caller_of)
// Going there, while the session's own process (the chain's first) still
// runs: the first process up the chain, still the same one, that has
// a window of its own (or a console window, kept by a conhost under it);
// with several (VS Code, Windows Terminal), the one whose title names the
// session or its project. Else the window its console belongs to: Windows
// Terminal hosts a console started outside it, and owns the console's
// hidden window. A session of the Claude desktop app also has the
// app open it (claude://code/continue?session=local_…), found by its id in
// the app's own records of its sessions.

use crate::state::Chain;

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

    #[repr(C)]
    struct UnicodeString {
        length: u16,
        maximum_length: u16,
        buffer: *mut u16,
    }

    #[link(name = "ntdll")]
    extern "system" {
        fn NtQueryInformationProcess(
            process: Handle,
            info_class: u32,
            info: *mut c_void,
            info_len: u32,
            return_len: *mut u32,
        ) -> i32;
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
        fn GetLastActivePopup(hwnd: Hwnd) -> Hwnd;
        fn PostMessageW(hwnd: Hwnd, msg: u32, wparam: usize, lparam: isize) -> i32;
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
    const WM_SYSCOMMAND: u32 = 0x0112;
    const SC_RESTORE: usize = 0xF120;

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

    // When a process started (100 ns since 1601), or None when gone or not
    // ours to ask. One that has ended but whose handle someone still holds
    // (a terminal's, a test's) can still be opened and still has its
    // times: its exit time set, it is gone too.
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
            let running = exited.low == 0 && exited.high == 0;
            (ok != 0 && running).then(|| (u64::from(created.high) << 32) | u64::from(created.low))
        }
    }

    // A process and the ones above it, up to what every program runs under.
    // A parent never starts after its child: one that did has a reused number.
    pub fn chain_of(pid: u32) -> Chain {
        named_chain(pid, &processes()).into_iter().map(|(pid, at, _)| (pid, at)).collect()
    }

    // The chain of a Codex session, from inside its hook command: from Codex
    // up, past this command and the shell Codex runs it in. With the agent's
    // own exe (`host`) up the chain, from the nearest of it: a hook of ours
    // handed on by a wrapper of the person's (a node script, gone once it
    // has handed the event on) is not the session's process
    // (zhoupengjie/Wakuwaku#75).
    pub fn hook_chain(host: Option<&str>) -> Chain {
        chain_from_hook(named_chain(std::process::id(), &processes()), host)
    }

    // hook_chain's cut, over the chain from the hook itself up.
    pub(super) fn chain_from_hook(named: Vec<(u32, u64, String)>, host: Option<&str>) -> Chain {
        let host = host.filter(|h| named.iter().skip(1).any(|(_, _, name)| name.eq_ignore_ascii_case(h)));
        named
            .into_iter()
            .skip(1)
            .skip_while(|(_, _, name)| match host {
                Some(h) => !name.eq_ignore_ascii_case(h),
                None => SHELLS.contains(&name.as_str()),
            })
            .map(|(pid, at, _)| (pid, at))
            .collect()
    }

    // The names of the processes above this one, the nearest first, `max` at
    // most, for her log (what started her, or a hook of hers: starts.rs):
    // none past one gone, or one whose number was reused (it started after
    // its child), or one not ours to ask when it started.
    pub fn parent_names(max: usize) -> Vec<String> {
        let all = processes();
        let mut out = Vec::new();
        let (mut cur, mut child_at) = (std::process::id(), started(std::process::id()));
        while out.len() < max {
            let Some(&(parent, _)) = all.get(&cur) else { break };
            if parent == 0 || parent == cur {
                break;
            }
            let Some((_, name)) = all.get(&parent) else { break };
            let at = started(parent);
            if at.zip(child_at).is_some_and(|(at, child)| at > child) {
                break;
            }
            out.push(name.clone());
            if at.is_none() {
                break;
            }
            (cur, child_at) = (parent, at);
        }
        out
    }

    // The outermost process above one, by the processes' list (their numbers
    // and parents): the app a hook runs under, past the shell and the hook
    // runners that are gone a moment later (DeepSeek Harness's: server.rs
    // caller_of). The app (its exe's name, `app`) ends the climb at its
    // outermost process of that name (an Electron app's helpers bear its
    // name too): what started it (a terminal, a script, Explorer) is not
    // where the session is (DSH started from a shell went to the shell's
    // console, seen 2026-10-16). A parent that started after its child,
    // where both may be asked, has a reused number. Itself when nothing is
    // above it.
    pub fn outermost_of(pid: u32, app: &str) -> u32 {
        outermost_in(&processes(), pid, app, started)
    }

    // outermost_of's climb over a list of processes and their start times.
    pub(super) fn outermost_in(all: &HashMap<u32, (u32, String)>, pid: u32, app: &str, started: impl Fn(u32) -> Option<u64>) -> u32 {
        let is_app = |p: u32| all.get(&p).is_some_and(|(_, name)| name.eq_ignore_ascii_case(app));
        let (mut cur, mut outermost) = (pid, pid);
        for _ in 0..MAX_CHAIN {
            let Some(&(parent, _)) = all.get(&cur) else { break };
            let Some((_, name)) = all.get(&parent) else { break };
            if parent == 0 || parent == cur || ROOTS.contains(&name.as_str()) || (is_app(cur) && !is_app(parent)) {
                break;
            }
            if let (Some(child), Some(at)) = (started(cur), started(parent)) {
                if at > child {
                    break;
                }
            }
            outermost = parent;
            cur = parent;
        }
        outermost
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

    // The TCP table's rows with their processes: { state, local address,
    // local port, remote address, remote port, pid }, ports as numbers.
    fn tcp_rows() -> Vec<[u32; 6]> {
        let port = |v: u32| u32::from(u16::from_be(v as u16));
        let mut size = 0u32;
        // SAFETY: first asking the size, then filling a buffer of that size.
        unsafe {
            GetExtendedTcpTable(std::ptr::null_mut(), &mut size, 0, AF_INET, TCP_TABLE_OWNER_PID_ALL, 0);
            let mut table = vec![0u32; size as usize / 4 + 64];
            size = (table.len() * 4) as u32;
            if GetExtendedTcpTable(table.as_mut_ptr().cast(), &mut size, 0, AF_INET, TCP_TABLE_OWNER_PID_ALL, 0) != 0 {
                return Vec::new();
            }
            let count = table[0] as usize;
            table[1..].chunks_exact(6).take(count).map(|r| [r[0], r[1], port(r[2]), r[3], port(r[4]), r[5]]).collect()
        }
    }

    // The processes connected to one that serves them on this computer
    // (OpenCode's background service: its TUIs and runs): those on the other
    // end of a connection to a port it listens on. Each once, itself never.
    fn clients_of(server: u32) -> Vec<u32> {
        const LISTEN: u32 = 2;
        const ESTABLISHED: u32 = 5;
        let rows = tcp_rows();
        let ports: Vec<u32> = rows.iter().filter(|r| r[5] == server && r[0] == LISTEN).map(|r| r[2]).collect();
        let mut out: Vec<u32> = Vec::new();
        for r in rows.iter().filter(|r| r[0] == ESTABLISHED && r[5] != server && ports.contains(&r[4]) && r[3] == u32::from_ne_bytes([127, 0, 0, 1])) {
            if !out.contains(&r[5]) {
                out.push(r[5]);
            }
        }
        out
    }

    // Where a session OpenCode's service runs is shown: the chain of the
    // program connected to it whose window's title names the session or its
    // project, else of the one started last that has a window (a run just
    // begun, the one TUI), else of the one started last; with none connected
    // (one run standalone serves itself), the server's own.
    pub fn front_chain(server: u32, hints: &[&str]) -> Chain {
        let all = processes();
        let chain = |pid: u32| -> Chain { named_chain(pid, &all).into_iter().map(|(pid, at, _)| (pid, at)).collect() };
        let mut chains: Vec<Chain> = clients_of(server).into_iter().map(chain).filter(|c| !c.is_empty()).collect();
        if chains.is_empty() {
            return chain(server);
        }
        // The newest first.
        chains.sort_by_key(|c| std::cmp::Reverse(c[0].1));
        let windows = windows();
        let shown: Vec<(usize, isize)> = chains.iter().enumerate().filter_map(|(i, c)| window_in(c, hints, true, &all, &windows).map(|w| (i, w))).collect();
        let named = shown.iter().find(|(_, hwnd)| {
            windows.iter().find(|w| w.hwnd == *hwnd).is_some_and(|w| hints.iter().any(|h| !h.is_empty() && w.title.contains(h)))
        });
        let at = named.or(shown.first()).map_or(0, |(i, _)| *i);
        chains.swap_remove(at)
    }

    // Whether something listens on 127.0.0.1:port (or every address) now, as
    // Windows' table of them says: a hook's way to know she is up without
    // trying her port, which Windows gives up on only after 2 s when no one
    // listens. Can't tell: true (try).
    pub fn is_listening(port: u16) -> bool {
        const TCP_TABLE_BASIC_LISTENER: u32 = 0;
        let mut size = 0u32;
        // SAFETY: first asking the size, then filling a buffer of that size.
        unsafe {
            GetExtendedTcpTable(std::ptr::null_mut(), &mut size, 0, AF_INET, TCP_TABLE_BASIC_LISTENER, 0);
            let mut table = vec![0u32; size as usize / 4 + 64];
            size = (table.len() * 4) as u32;
            if GetExtendedTcpTable(table.as_mut_ptr().cast(), &mut size, 0, AF_INET, TCP_TABLE_BASIC_LISTENER, 0) != 0 {
                return true;
            }
            // { count, then rows of state, local address, local port, remote address, remote port }
            let count = table[0] as usize;
            let local = |addr: u32| addr == 0 || addr == u32::from_ne_bytes([127, 0, 0, 1]);
            table[1..].chunks_exact(5).take(count).any(|row| u16::from_be(row[2] as u16) == port && local(row[1]))
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
        window_in(chain, hints, needs_own, &processes(), &windows())
    }

    // The windows of several sessions at once: the processes and the
    // windows listed once for all of them (listed again for each session,
    // every three seconds, they were most of the taskbar's idle work). Each
    // found window comes with its process, to see later that it is still
    // the same (still_there).
    pub fn windows_for(asks: &[(Chain, Vec<String>)]) -> Vec<Option<(isize, u32)>> {
        if asks.is_empty() {
            return Vec::new();
        }
        let all = processes();
        let windows = windows();
        asks.iter()
            .map(|(chain, hints)| {
                let hints: Vec<&str> = hints.iter().map(String::as_str).collect();
                window_in(chain, &hints, true, &all, &windows).map(|hwnd| (hwnd, pid_of_window(hwnd)))
            })
            .collect()
    }

    fn pid_of_window(hwnd: isize) -> u32 {
        let mut pid = 0u32;
        // SAFETY: our own out-parameter.
        unsafe { GetWindowThreadProcessId(hwnd as Hwnd, &mut pid) };
        pid
    }

    // A session's own process still the one it was: running, started when
    // the chain says (its number not reused).
    pub fn still_running(pid: u32, at: u64) -> bool {
        started(pid) == Some(at)
    }

    // When a process started, in ms since 1970 (a FILETIME counts 100 ns
    // since 1601); None when gone or not ours to ask.
    pub fn started_ms(pid: u32) -> Option<u64> {
        started(pid).map(|at| (at / 10_000).saturating_sub(11_644_473_600_000))
    }

    // When each running process of this exe's name started (ms since 1970),
    // those that may be asked: whether an app runs, and since when
    // (connection/dsh.rs dsh_view).
    pub fn started_named(exe: &str) -> Vec<u64> {
        let exe = exe.to_lowercase();
        processes().into_iter().filter(|(_, (_, name))| *name == exe).filter_map(|(pid, _)| started_ms(pid)).collect()
    }

    // A window found before is still the one: there, shown, of the same process.
    pub fn still_there(hwnd: isize, pid: u32) -> bool {
        // SAFETY: plain queries.
        let shown = unsafe { IsWindowVisible(hwnd as Hwnd) != 0 };
        shown && pid_of_window(hwnd) == pid
    }

    fn window_in(chain: &Chain, hints: &[&str], needs_own: bool, all: &HashMap<u32, (u32, String)>, windows: &[Window]) -> Option<isize> {
        // Its own process gone, the session is over: no window is its.
        let &(first, at) = chain.first()?;
        if needs_own && started(first) != Some(at) {
            return None;
        }
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

    // A session whose app keeps its window in the tray (Codex's desktop app,
    // hidden there: no window to find), its own process still running: the
    // app started again by its app id, the first one up the chain (still
    // the same) that Windows knows as a packaged app's, below any shell
    // (above one is the terminal the agent runs in, which would open a new
    // window); one of it at a time, it shows the window it has. That window
    // then looked for, 2 s at most (zhoupengjie/Wakuwaku#75).
    pub fn wake(chain: &Chain, hints: &[&str]) -> Option<isize> {
        let &(first, at) = chain.first()?;
        if started(first) != Some(at) {
            return None;
        }
        let all = processes();
        let id = chain
            .iter()
            .take_while(|(pid, _)| !all.get(pid).is_some_and(|(_, name)| SHELLS.contains(&name.as_str())))
            .filter(|&&(pid, at)| started(pid) == Some(at))
            .find_map(|&(pid, _)| crate::tasks::process_app_id(pid))?;
        crate::tasks::with_com();
        if !open_url(&format!("shell:AppsFolder\\{id}")) {
            return None;
        }
        let asked = std::time::Instant::now();
        while asked.elapsed() < std::time::Duration::from_secs(2) {
            std::thread::sleep(std::time::Duration::from_millis(100));
            if let Some(hwnd) = window_of(chain, hints, true) {
                return Some(hwnd);
            }
        }
        None
    }

    // The window a process's console shows in: the terminal's that hosts it
    // (it owns the console's window), or the console's own. Never
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
            // A terminal's that owns it first: Windows Terminal 1.24's says
            // it is visible too (seen in the test VM, 2026-10-15), and to the
            // front it is nothing to see, its terminal left minimized.
            let owner = GetAncestor(console, GA_ROOTOWNER);
            if !owner.is_null() && owner != console && IsWindowVisible(owner) != 0 {
                return Some(owner as isize);
            }
            (IsWindowVisible(console) != 0).then_some(console as isize)
        }
    }

    // To the front, restored if minimized. Windows hands the foreground only
    // to the one who has it (or was last clicked); else borrow its input.
    pub fn bring(hwnd: isize) -> bool {
        let h = hwnd as Hwnd;
        // SAFETY: a window handle just found; a stale one only makes the calls fail.
        unsafe {
            if IsIconic(h) != 0 {
                // Asked to restore itself, as Windows' taskbar and Win+Tab
                // ask: a program drawing its own frame (WeGame, Xunyou) stays
                // minimized when restored from outside. Some take a while
                // (WeGame, past 300 ms), and a restore forced meanwhile undoes
                // theirs: forced only once it has had its time.
                PostMessageW(h, WM_SYSCOMMAND, SC_RESTORE, 0);
                let asked = std::time::Instant::now();
                while IsIconic(h) != 0 && asked.elapsed() < std::time::Duration::from_millis(1500) {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                if IsIconic(h) != 0 {
                    ShowWindow(h, SW_RESTORE);
                }
            }
            // Its dialog, should one be open in it, as Windows' taskbar does.
            let popup = GetLastActivePopup(h);
            let h = if !popup.is_null() && IsWindowVisible(popup) != 0 { popup } else { h };
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

    // Read a process's command line (Windows 8.1+ ProcessCommandLineInformation = 60).
    pub fn cmdline_of(pid: u32) -> Option<String> {
        unsafe {
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if process == 0 {
                return None;
            }
            let mut ret_len = 0u32;
            let mut buf = vec![0u8; 4096];
            let mut status = NtQueryInformationProcess(process, 60, buf.as_mut_ptr().cast(), buf.len() as u32, &mut ret_len);
            if (status as u32 == 0xC0000023 || ret_len > buf.len() as u32) && ret_len > 0 {
                buf.resize(ret_len as usize + 64, 0);
                status = NtQueryInformationProcess(process, 60, buf.as_mut_ptr().cast(), buf.len() as u32, &mut ret_len);
            }
            CloseHandle(process);
            if status != 0 || ret_len < std::mem::size_of::<UnicodeString>() as u32 {
                return None;
            }
            let us = &*(buf.as_ptr() as *const UnicodeString);
            if us.buffer.is_null() || us.length == 0 {
                return None;
            }
            let slice = std::slice::from_raw_parts(us.buffer, (us.length / 2) as usize);
            Some(String::from_utf16_lossy(slice))
        }
    }

    // Check whether the hook's process chain includes `agy` (CLI), and return any flags found in its command line.
    pub fn agy_cli_info() -> Option<(u32, String)> {
        let all = processes();
        let named = named_chain(std::process::id(), &all);
        for &(pid, _, ref name) in &named {
            let n = name.to_lowercase();
            if n == "agy.exe" || n == "agy" {
                let cmd = cmdline_of(pid).unwrap_or_default();
                return Some((pid, cmd));
            }
            if n == "node.exe" || n == "node" {
                if let Some(cmd) = cmdline_of(pid) {
                    if cmd.contains("antigravity-cli") || cmd.contains("agy") {
                        return Some((pid, cmd));
                    }
                }
            }
        }
        None
    }
}

#[cfg(not(windows))]
mod imp {
    use super::Chain;

    pub fn chain_of(_pid: u32) -> Chain {
        Vec::new()
    }
    pub fn hook_chain(_host: Option<&str>) -> Chain {
        Vec::new()
    }
    pub fn parent_names(_max: usize) -> Vec<String> {
        Vec::new()
    }
    pub fn outermost_of(pid: u32, _app: &str) -> u32 {
        pid
    }
    pub fn client_of(_client_port: u16, _server_port: u16) -> Option<u32> {
        None
    }
    pub fn front_chain(_server: u32, _hints: &[&str]) -> Chain {
        Vec::new()
    }
    pub fn is_listening(_port: u16) -> bool {
        true
    }
    pub fn window_of(_chain: &Chain, _hints: &[&str], _needs_own: bool) -> Option<isize> {
        None
    }
    pub fn wake(_chain: &Chain, _hints: &[&str]) -> Option<isize> {
        None
    }
    pub fn windows_for(asks: &[(Chain, Vec<String>)]) -> Vec<Option<(isize, u32)>> {
        asks.iter().map(|_| None).collect()
    }
    pub fn still_there(_hwnd: isize, _pid: u32) -> bool {
        false
    }
    pub fn started_ms(_pid: u32) -> Option<u64> {
        None
    }
    pub fn started_named(_exe: &str) -> Vec<u64> {
        Vec::new()
    }
    pub fn still_running(_pid: u32, _at: u64) -> bool {
        false
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
    pub fn cmdline_of(_pid: u32) -> Option<String> {
        None
    }
    pub fn agy_cli_info() -> Option<(u32, String)> {
        None
    }
}

pub use imp::{agy_cli_info, chain_of, client_of, front_chain, hook_chain, is_listening, outermost_of, parent_names, started_ms, started_named, still_running, still_there, windows_for};

// A window to the front, restored if minimized (the taskbar's buttons).
pub fn bring_window(hwnd: isize) -> bool {
    imp::bring(hwnd)
}

// A web address opened in the default browser, as Windows opens one
// (ShellExecute; no console of ours in between).
pub fn open_url(url: &str) -> bool {
    imp::open_url(url)
}

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

// To the session: its window to the front, its app woken from the tray
// first should its window be there; for a session of the Claude desktop
// app, the app opens it too. True when anything was found to go to.
pub fn go(session: &str, chain: &Chain, hints: &[&str]) -> bool {
    // The desktop app's session is the app's to show, its process running or not.
    let desktop = imp::desktop_session(session);
    let window = imp::window_of(chain, hints, desktop.is_none()).or_else(|| desktop.is_none().then(|| imp::wake(chain, hints)).flatten());
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
    fn when_a_process_started() {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64;
        let at = started_ms(std::process::id()).unwrap();
        assert!(at <= now && now - at < 3_600_000, "{at} {now}");
        // This test's own exe, by its name, among them.
        let exe = std::env::current_exe().unwrap();
        assert!(started_named(&exe.file_name().unwrap().to_string_lossy().to_uppercase()).contains(&at));
        assert!(started_named("no such app.exe").is_empty());
        assert_eq!(started_ms(u32::MAX - 3), None);
    }

    #[cfg(windows)]
    #[test]
    fn the_outermost_process_above_this_one() {
        // Above this test: cargo, a shell, a terminal… up to what all run under.
        let top = outermost_of(std::process::id(), "no such app.exe");
        assert_ne!(top, std::process::id());
        assert!(chain_of(std::process::id()).iter().any(|&(pid, _)| pid == top) || chain_of(top).len() == 1);
        // None such: itself.
        assert_eq!(outermost_of(u32::MAX - 3, "no such app.exe"), u32::MAX - 3);
    }

    // DeepSeek Harness's hook (wakuwaku ← pwsh ← runner ← host ← the app,
    // its helpers bearing its name) climbs to the app's outermost process
    // and no further, whoever started the app; with no app above it, up to
    // what all run under.
    #[cfg(windows)]
    #[test]
    fn the_climb_stops_at_the_app() {
        use std::collections::HashMap;
        let app = "DeepSeek Harness.exe";
        let list = |pairs: &[(u32, u32, &str)]| pairs.iter().map(|&(pid, parent, name)| (pid, (parent, name.to_string()))).collect::<HashMap<_, _>>();
        // Each started after its parent (a higher number here).
        let started = |pid: u32| Some(u64::from(pid) * 10);
        let from_explorer = list(&[(9, 8, "wakuwaku.exe"), (8, 7, "pwsh.exe"), (7, 6, "deepseek harness.exe"), (6, 5, "deepseek harness.exe"), (5, 4, "explorer.exe")]);
        assert_eq!(super::imp::outermost_in(&from_explorer, 9, app, started), 6);
        let from_a_shell = list(&[(9, 8, "wakuwaku.exe"), (8, 7, "pwsh.exe"), (7, 6, "deepseek harness.exe"), (6, 5, "deepseek harness.exe"), (5, 4, "pwsh.exe"), (4, 3, "windowsterminal.exe"), (3, 2, "explorer.exe")]);
        assert_eq!(super::imp::outermost_in(&from_a_shell, 9, app, started), 6);
        let no_app = list(&[(9, 8, "wakuwaku.exe"), (8, 7, "pwsh.exe"), (7, 6, "cmd.exe"), (6, 5, "explorer.exe")]);
        assert_eq!(super::imp::outermost_in(&no_app, 9, app, started), 7);
    }

    // Codex's hook (wakuwaku ← shell ← codex): its chain from Codex up; a
    // wrapper of the person's between (wakuwaku ← node ← pwsh ← codex,
    // zhoupengjie/Wakuwaku#75) cut away too. No Codex above, or no host
    // asked: past the shells only, as before.
    #[cfg(windows)]
    #[test]
    fn a_hook_chain_starts_at_its_host() {
        let named = |names: &[&str]| names.iter().enumerate().map(|(i, n)| (100 - i as u32, 1_000 - i as u64, n.to_string())).collect::<Vec<_>>();
        let pids = |chain: Chain| chain.into_iter().map(|(pid, _)| pid).collect::<Vec<_>>();
        let plain = named(&["wakuwaku.exe", "pwsh.exe", "codex.exe", "codex.exe"]);
        assert_eq!(pids(super::imp::chain_from_hook(plain.clone(), Some("codex.exe"))), [98, 97]);
        assert_eq!(pids(super::imp::chain_from_hook(plain, None)), [98, 97]);
        let wrapped = named(&["wakuwaku.exe", "node.exe", "pwsh.exe", "codex.exe", "codex.exe"]);
        assert_eq!(pids(super::imp::chain_from_hook(wrapped.clone(), Some("codex.exe"))), [97, 96]);
        assert_eq!(pids(super::imp::chain_from_hook(wrapped, None)), [99, 98, 97, 96]);
        let no_host = named(&["wakuwaku.exe", "node.exe", "cmd.exe", "windowsterminal.exe"]);
        assert_eq!(pids(super::imp::chain_from_hook(no_host, Some("codex.exe"))), [99, 98, 97]);
    }

    #[cfg(windows)]
    #[test]
    fn a_port_listened_on_is_seen() {
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        assert!(is_listening(port));
        drop(listener);
        assert!(!cfg!(windows) || !is_listening(port));
    }

    // How long a hook's look at where it runs takes (cargo test -- --ignored --nocapture).
    #[test]
    #[ignore]
    fn how_long_a_hook_chain_takes() {
        let t = std::time::Instant::now();
        for _ in 0..20 {
            let _ = hook_chain(None);
        }
        println!("hook_chain: {:?} each", t.elapsed() / 20);
        let t = std::time::Instant::now();
        for _ in 0..20 {
            let _ = is_listening(47213);
        }
        println!("is_listening: {:?} each", t.elapsed() / 20);
    }

    // Windows' TCP table: elsewhere client_of knows no connection's process.
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
