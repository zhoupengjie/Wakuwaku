// What she starts, and her process out of the job of whoever starts her.
// A session's hook (Claude Code's, Codex's, Antigravity's) runs in its
// agent's job, and what a desktop app starts may stay in the app's job,
// which ends everything in it as the app quits: started from Claude's
// desktop app, she and her taskbar's guard went with it as it updated, and
// Windows' taskbar was left put away on every display until she was started
// again (the user, 2026-10-13). Breaking away is up to the job, and that one
// would not let her.
//
// start: a program started as any program starts another (CreateProcess):
// detached, inheriting no handle (a hook's process holds the ones its agent
// left inheritable, its output pipes among them, and whoever reads those to
// the end would wait on her for good), in its own folder; out of her job
// when it lets what she starts go (CREATE_BREAKAWAY_FROM_JOB), else in it.
// Its process is kept, to be waited on or ended (an update's new version:
// updates.rs). An update, a restart and the taskbar's guard start so: by
// then her job is Explorer's or none (should_leave saw to that as she
// started), and theirs is hers.
//
// spawn: out of the starter's job, where staying in it would end her: a
// session's hook starting her, and should_leave. As start when there is no
// job or it lets go; only when it holds on to what is started in it (as
// Claude's desktop app's does) with Explorer as the parent
// (PROC_THREAD_ATTRIBUTE_PARENT_PROCESS), whose job (none) what it starts
// goes in, as anything opened from the desktop does. Another process
// borrowed as the parent is also how malware hides where it came from
// (ATT&CK T1134.004), and antiviruses watch for it: Kaspersky's behaviour
// protection took her new version away as an update of hers started it so
// (zhoupengjie/Wakuwaku#48). Explorer not there, or refusing: as start.
//
// held_elsewhere: in a job that holds on to what she starts and is not
// Explorer's (Explorer is in none): run from the source tree in a
// terminal's or an app's, or should_leave's start refused. Her taskbar's
// guard is spawned then, as a job's end would take it with her.
//
// should_leave: in a job and not started by Explorer (a hook of an older
// her, a terminal, an app's own shell): she starts herself again through
// spawn, once (FLAG), and this copy goes. Not run from the source tree.

pub const FLAG: &str = "--wakuwaku-out-of-job";

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;
    use crate::win32::{ProcessEntry, CloseHandle, CreateToolhelp32Snapshot, GetCurrentProcess, GetCurrentProcessId, GetExitCodeProcess, GetShellWindow, GetWindowThreadProcessId, OpenProcess, Process32FirstW, Process32NextW, TerminateProcess, WaitForSingleObject};

    #[repr(C)]
    struct StartupInfo {
        cb: u32,
        reserved: *mut u16,
        desktop: *mut u16,
        title: *mut u16,
        x: u32,
        y: u32,
        x_size: u32,
        y_size: u32,
        x_chars: u32,
        y_chars: u32,
        fill: u32,
        flags: u32,
        show: u16,
        reserved2_len: u16,
        reserved2: *mut u8,
        stdin: isize,
        stdout: isize,
        stderr: isize,
    }

    #[repr(C)]
    struct StartupInfoEx {
        info: StartupInfo,
        attributes: *mut c_void,
    }

    #[repr(C)]
    struct ProcessInfo {
        process: isize,
        thread: isize,
        process_id: u32,
        thread_id: u32,
    }

    // JOBOBJECT_BASIC_LIMIT_INFORMATION and the extended one around it,
    // which alone carries the breakaway limits.
    #[repr(C)]
    struct BasicLimits {
        per_process_user_time: i64,
        per_job_user_time: i64,
        flags: u32,
        min_working_set: usize,
        max_working_set: usize,
        active_processes: u32,
        affinity: usize,
        priority_class: u32,
        scheduling_class: u32,
    }

    #[repr(C)]
    struct ExtendedLimits {
        basic: BasicLimits,
        io: [u64; 6],
        process_memory: usize,
        job_memory: usize,
        peak_process_memory: usize,
        peak_job_memory: usize,
    }

    #[link(name = "kernel32")]
    extern "system" {
        #[allow(clippy::too_many_arguments)]
        fn CreateProcessW(
            app: *const u16,
            command_line: *mut u16,
            process_attrs: *const u8,
            thread_attrs: *const u8,
            inherit_handles: i32,
            flags: u32,
            env: *const u8,
            dir: *const u16,
            startup: *const StartupInfo,
            info: *mut ProcessInfo,
        ) -> i32;
        fn IsProcessInJob(process: isize, job: isize, result: *mut i32) -> i32;
        fn QueryInformationJobObject(job: isize, class: i32, info: *mut c_void, length: u32, returned: *mut u32) -> i32;
        fn InitializeProcThreadAttributeList(list: *mut c_void, count: u32, flags: u32, size: *mut usize) -> i32;
        fn UpdateProcThreadAttribute(list: *mut c_void, flags: u32, attribute: usize, value: *const c_void, size: usize, previous: *mut c_void, returned: *mut usize) -> i32;
        fn DeleteProcThreadAttributeList(list: *mut c_void);
    }

    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
    const EXTENDED_STARTUPINFO_PRESENT: u32 = 0x0008_0000;
    const PROCESS_CREATE_PROCESS: u32 = 0x0080;
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const PROC_THREAD_ATTRIBUTE_PARENT_PROCESS: usize = 0x0002_0000;
    const JOB_OBJECT_EXTENDED_LIMIT_INFORMATION: i32 = 9;
    const JOB_OBJECT_LIMIT_BREAKAWAY_OK: u32 = 0x0000_0800;
    const JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK: u32 = 0x0000_1000;
    const TH32CS_SNAPPROCESS: u32 = 0x2;
    const INVALID_HANDLE: isize = -1;
    const WAIT_OBJECT_0: u32 = 0;

    // A process she started, its handle held till dropped.
    pub struct Started {
        pub pid: u32,
        process: isize,
    }

    impl Started {
        // Its exit code, should it end within `ms`.
        pub fn ended_within(&mut self, ms: u32) -> Option<u32> {
            // SAFETY: our own handle; the code written into our own u32.
            unsafe {
                if WaitForSingleObject(self.process, ms) != WAIT_OBJECT_0 {
                    return None;
                }
                let mut code = 0u32;
                GetExitCodeProcess(self.process, &mut code);
                Some(code)
            }
        }

        pub fn end(&mut self) {
            // SAFETY: our own handle.
            unsafe { TerminateProcess(self.process, 1) };
        }
    }

    impl Drop for Started {
        fn drop(&mut self) {
            // SAFETY: the handle CreateProcess gave us, closed once.
            unsafe { CloseHandle(self.process) };
        }
    }

    fn wide(s: &std::ffi::OsStr) -> Vec<u16> {
        s.encode_wide().chain(Some(0)).collect()
    }

    // Explorer's process: the one with the shell's (desktop's) window.
    fn explorer() -> Option<u32> {
        let mut pid = 0;
        // SAFETY: plain queries; a missing window gives none.
        let shell = unsafe { GetShellWindow() };
        if shell.is_null() || unsafe { GetWindowThreadProcessId(shell, &mut pid) } == 0 || pid == 0 {
            return None;
        }
        Some(pid)
    }

    // A command line as CreateProcess takes it: each part quoted (no part of
    // hers holds a quote: paths, flags, numbers).
    fn command_line(exe: &Path, args: &[&str]) -> Vec<u16> {
        let mut line = format!("\"{}\"", exe.display());
        for a in args {
            line.push_str(&format!(" \"{a}\""));
        }
        wide(std::ffi::OsStr::new(&line))
    }

    // Her job: none, one that lets what she starts out of it, or one that
    // holds on to it (its limits unread count as that).
    #[derive(Clone, Copy, PartialEq)]
    enum Job {
        None,
        LetsGo,
        Holds,
    }

    fn job() -> Job {
        let mut in_job = 0;
        // SAFETY: a plain query about this process.
        if unsafe { IsProcessInJob(GetCurrentProcess(), 0, &mut in_job) } == 0 || in_job == 0 {
            return Job::None;
        }
        // SAFETY: no job handle asks of her own job; written into our own struct.
        let mut limits: ExtendedLimits = unsafe { std::mem::zeroed() };
        let read = unsafe { QueryInformationJobObject(0, JOB_OBJECT_EXTENDED_LIMIT_INFORMATION, (&mut limits as *mut ExtendedLimits).cast(), std::mem::size_of::<ExtendedLimits>() as u32, std::ptr::null_mut()) } != 0;
        if read && limits.basic.flags & (JOB_OBJECT_LIMIT_BREAKAWAY_OK | JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK) != 0 {
            Job::LetsGo
        } else {
            Job::Holds
        }
    }

    // Started plainly from here (out of her job, or in it); its process kept.
    fn create(exe: &Path, args: &[&str], breakaway: bool) -> std::io::Result<Started> {
        let app = wide(exe.as_os_str());
        let mut line = command_line(exe, args);
        // Its own folder, not the starter's (a hook's: the project's): it would hold on to it.
        let dir = exe.parent().map(|d| wide(d.as_os_str()));
        let dir = dir.as_ref().map_or(std::ptr::null(), |d| d.as_ptr());
        // SAFETY: plain Win32 structs, zero is a valid start for each.
        let mut startup: StartupInfo = unsafe { std::mem::zeroed() };
        startup.cb = std::mem::size_of::<StartupInfo>() as u32;
        let mut info: ProcessInfo = unsafe { std::mem::zeroed() };
        let flags = DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | if breakaway { CREATE_BREAKAWAY_FROM_JOB } else { 0 };
        let null = std::ptr::null();
        // SAFETY: every pointer is valid for the call or null where Win32 allows it.
        if unsafe { CreateProcessW(app.as_ptr(), line.as_mut_ptr(), null, null, 0, flags, null, dir, &startup, &mut info) } == 0 {
            return Err(std::io::Error::last_os_error());
        }
        // SAFETY: the thread's handle CreateProcess gave us, not needed.
        unsafe { CloseHandle(info.thread) };
        Ok(Started { pid: info.process_id, process: info.process })
    }

    pub fn start(exe: &Path, args: &[&str]) -> std::io::Result<Started> {
        match job() {
            // Out of it should it let go; else (a nested job refusing) in it.
            Job::LetsGo => create(exe, args, true).or_else(|_| create(exe, args, false)),
            _ => create(exe, args, false),
        }
    }

    // Explorer as the parent: a list of one attribute, its process.
    fn from_explorer(exe: &Path, args: &[&str]) -> bool {
        let Some(parent) = explorer().map(|pid| unsafe { OpenProcess(PROCESS_CREATE_PROCESS, 0, pid) }).filter(|h| *h != 0) else { return false };
        let app = wide(exe.as_os_str());
        let mut line = command_line(exe, args);
        let dir = exe.parent().map(|d| wide(d.as_os_str()));
        let dir = dir.as_ref().map_or(std::ptr::null(), |d| d.as_ptr());
        // SAFETY: plain Win32 structs, zero is a valid start for each.
        let mut startup: StartupInfoEx = unsafe { std::mem::zeroed() };
        startup.info.cb = std::mem::size_of::<StartupInfoEx>() as u32;
        let mut info: ProcessInfo = unsafe { std::mem::zeroed() };
        let null = std::ptr::null();
        let mut list: Vec<u8> = Vec::new();
        let mut size = 0usize;
        let mut made = false;
        // SAFETY: asked for its size first (the call fails, saying it), then
        // made in a buffer of that size; the parent's handle outlives the call.
        unsafe { InitializeProcThreadAttributeList(std::ptr::null_mut(), 1, 0, &mut size) };
        list.resize(size.max(1), 0);
        let ready = unsafe { InitializeProcThreadAttributeList(list.as_mut_ptr().cast(), 1, 0, &mut size) } != 0
            && unsafe { UpdateProcThreadAttribute(list.as_mut_ptr().cast(), 0, PROC_THREAD_ATTRIBUTE_PARENT_PROCESS, (&parent as *const isize).cast(), std::mem::size_of::<isize>(), std::ptr::null_mut(), std::ptr::null_mut()) } != 0;
        if ready {
            startup.attributes = list.as_mut_ptr().cast();
            // SAFETY: every pointer is valid for the call or null where Win32 allows it.
            made = unsafe { CreateProcessW(app.as_ptr(), line.as_mut_ptr(), null, null, 0, DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | EXTENDED_STARTUPINFO_PRESENT, null, dir, &startup.info, &mut info) } != 0;
            // SAFETY: the list was made above and is not used after.
            unsafe { DeleteProcThreadAttributeList(list.as_mut_ptr().cast()) };
        }
        // SAFETY: our own handles, and those CreateProcess gave us.
        unsafe {
            CloseHandle(parent);
            if made {
                CloseHandle(info.thread);
                CloseHandle(info.process);
            }
        }
        made
    }

    pub fn spawn(exe: &Path, args: &[&str]) -> bool {
        match job() {
            Job::None => create(exe, args, false).is_ok(),
            Job::LetsGo => create(exe, args, true).is_ok() || from_explorer(exe, args) || create(exe, args, false).is_ok(),
            Job::Holds => from_explorer(exe, args) || create(exe, args, false).is_ok(),
        }
    }

    pub fn held_elsewhere() -> bool {
        if job() != Job::Holds {
            return false;
        }
        let Some(explorer) = explorer() else { return true };
        let mut in_job = 0;
        // SAFETY: a handle we close; a plain query about that process.
        unsafe {
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, explorer);
            if process == 0 {
                return true;
            }
            IsProcessInJob(process, 0, &mut in_job);
            CloseHandle(process);
        }
        in_job == 0
    }

    // The process that started this one (by a snapshot of them all).
    fn parent_id() -> Option<u32> {
        // SAFETY: a snapshot we close; entries written into our own struct.
        unsafe {
            let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snap == INVALID_HANDLE || snap == 0 {
                return None;
            }
            let me = GetCurrentProcessId();
            let mut entry: ProcessEntry = std::mem::zeroed();
            entry.size = std::mem::size_of::<ProcessEntry>() as u32;
            let mut found = None;
            let mut ok = Process32FirstW(snap, &mut entry) != 0;
            while ok {
                if entry.pid == me {
                    found = Some(entry.parent);
                    break;
                }
                ok = Process32NextW(snap, &mut entry) != 0;
            }
            CloseHandle(snap);
            found
        }
    }

    pub fn should_leave() -> bool {
        if job() == Job::None {
            return false;
        }
        match (explorer(), parent_id()) {
            (Some(explorer), Some(parent)) => parent != explorer,
            _ => false,
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use std::path::Path;
    use std::time::{Duration, Instant};

    pub struct Started {
        pub pid: u32,
        child: std::process::Child,
    }

    impl Started {
        pub fn ended_within(&mut self, ms: u32) -> Option<u32> {
            let until = Instant::now() + Duration::from_millis(ms.into());
            loop {
                if let Ok(Some(status)) = self.child.try_wait() {
                    return Some(status.code().unwrap_or(-1) as u32);
                }
                if Instant::now() >= until {
                    return None;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }

        pub fn end(&mut self) {
            let _ = self.child.kill();
        }
    }

    pub fn start(exe: &Path, args: &[&str]) -> std::io::Result<Started> {
        let child = std::process::Command::new(exe)
            .args(args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()?;
        Ok(Started { pid: child.id(), child })
    }

    pub fn spawn(exe: &Path, args: &[&str]) -> bool {
        start(exe, args).is_ok()
    }

    pub fn held_elsewhere() -> bool {
        false
    }

    pub fn should_leave() -> bool {
        false
    }
}

pub use imp::{held_elsewhere, should_leave, spawn, start};
