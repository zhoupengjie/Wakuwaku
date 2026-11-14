// Her start as the user logs on, through Windows' Task Scheduler (beside
// the Run value: start at login, below), and when this session's
// logon was (her start logs how long after it she came up,
// zhoupengjie/Wakuwaku#44).
//
// The task: one for each user (the Task Scheduler's root folder is the
// whole computer's, so its name ends in the user's SID), started by this
// user's logon alone (anyone's logon would need an administrator), with this
// user's own rights, as they log on (no delay). Normal priority (a task's
// own is below normal, which her process would keep), on battery as on the
// mains (a task's own does not start on battery, and ends as the plug comes
// out), and no time limit (a task's own ends after three days). Written,
// read and removed as the Task Scheduler's XML through its COM objects
// (ITaskService): no schtasks.exe, which says its XML in the console's code
// page, not a path's every letter.
//
// Off Windows there is neither a task nor a logon read: what is here is
// Windows' (and its tests').
#![cfg_attr(not(windows), allow(dead_code))]

// The task as read: what it runs, with what, and whether it is on (anything
// in it turned off, in the Task Scheduler: the task or its trigger).
#[derive(Debug, PartialEq)]
pub struct TaskSeen {
    pub program: String,
    pub args: String,
    pub enabled: bool,
}

const DESCRIPTION: &str = "Starts Wakuwaku as you log on: her Start at login. She keeps it to where her program is, and takes it away when Start at login is turned off. 登录时启动 Wakuwaku（她的「开机自动启动」）。";

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn unescape(text: &str) -> String {
    text.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}

// The task for this user (`user`: their SID), running `exe` with `args`.
pub fn task_xml(user: &str, exe: &str, args: &str) -> String {
    let (user, exe, args) = (escape(user), escape(exe), escape(args));
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Author>Wakuwaku</Author>
    <Description>{DESCRIPTION}</Description>
  </RegistrationInfo>
  <Triggers>
    <LogonTrigger>
      <Enabled>true</Enabled>
      <UserId>{user}</UserId>
    </LogonTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <UserId>{user}</UserId>
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>LeastPrivilege</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Priority>4</Priority>
    <Enabled>true</Enabled>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{exe}</Command>
      <Arguments>{args}</Arguments>
    </Exec>
  </Actions>
</Task>
"#
    )
}

// An element's text, the first of its name (none: not there; empty as <x/>).
fn text_of(xml: &str, tag: &str) -> Option<String> {
    if xml.contains(&format!("<{tag}/>")) || xml.contains(&format!("<{tag} />")) {
        return Some(String::new());
    }
    let start = xml.find(&format!("<{tag}>"))? + tag.len() + 2;
    let end = start + xml[start..].find(&format!("</{tag}>"))?;
    Some(unescape(xml[start..end].trim()))
}

// What a task's XML (as the Task Scheduler gives it back) runs.
pub fn seen(xml: &str) -> TaskSeen {
    let program = text_of(xml, "Command").unwrap_or_default();
    TaskSeen {
        program: program.trim_matches('"').to_string(),
        args: text_of(xml, "Arguments").unwrap_or_default(),
        enabled: !xml.contains("<Enabled>false</Enabled>"),
    }
}

// How long ago that was, as a person reads it: tenths of a second at first.
pub fn how_long(seconds: f64) -> String {
    let s = seconds.max(0.0);
    if s < 120.0 {
        format!("{s:.1} s")
    } else if s < 7200.0 {
        format!("{} min", (s / 60.0).round() as u64)
    } else {
        let m = (s / 60.0).round() as u64;
        format!("{} h {} min", m / 60, m % 60)
    }
}

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;
    use std::ptr::null_mut;
    use crate::win32::{Guid, CloseHandle, CoCreateInstance, CoInitializeEx, CoUninitialize, GetCurrentProcess, GetTokenInformation, LocalFree, OpenProcessToken};

    const CLSID_TASK_SCHEDULER: Guid = Guid(0x0F87_369F, 0xA4E5, 0x4CFC, [0xBD, 0x3E, 0x73, 0xE6, 0x15, 0x45, 0x72, 0xDD]);
    const IID_ITASK_SERVICE: Guid = Guid(0x2FAB_A4C7, 0x4DA9, 0x4013, [0x96, 0x97, 0x20, 0xCC, 0x3F, 0xD4, 0x0F, 0x85]);
    const COINIT_MULTITHREADED: u32 = 0x0;
    const CLSCTX_INPROC_SERVER: u32 = 0x1;
    const TASK_CREATE_OR_UPDATE: i32 = 6;
    const TASK_LOGON_INTERACTIVE_TOKEN: i32 = 3;
    // No such task: HRESULT_FROM_WIN32 of ERROR_FILE_NOT_FOUND, ERROR_PATH_NOT_FOUND.
    const NOT_FOUND: [u32; 2] = [0x8007_0002, 0x8007_0003];
    const TOKEN_QUERY: u32 = 0x0008;
    const TOKEN_USER: i32 = 1;
    const WTS_CURRENT_SESSION: u32 = 0xFFFF_FFFF;
    const WTS_SESSION_INFO: i32 = 24;

    type Bstr = *mut u16;

    // A VARIANT left empty (VT_EMPTY, all zeros): each one these calls take.
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Variant {
        vt: u16,
        reserved: [u16; 3],
        data: [usize; 2],
    }

    const EMPTY: Variant = Variant { vt: 0, reserved: [0; 3], data: [0; 2] };

    #[repr(C)]
    struct Obj {
        vtbl: *const c_void,
    }

    // IUnknown's three and IDispatch's four, which each of the three begins with.
    #[repr(C)]
    struct Dispatch {
        _query_interface: usize,
        _add_ref: usize,
        release: unsafe extern "system" fn(*mut Obj) -> u32,
        _get_type_info_count: usize,
        _get_type_info: usize,
        _get_ids_of_names: usize,
        _invoke: usize,
    }

    // ITaskService, ITaskFolder, IRegisteredTask, as far as what is asked
    // of them (the rest's places kept).
    #[repr(C)]
    struct ServiceVtbl {
        _base: Dispatch,
        get_folder: unsafe extern "system" fn(*mut Obj, Bstr, *mut *mut Obj) -> i32,
        _get_running_tasks: usize,
        _new_task: usize,
        connect: unsafe extern "system" fn(*mut Obj, Variant, Variant, Variant, Variant) -> i32,
    }

    #[repr(C)]
    struct FolderVtbl {
        _base: Dispatch,
        _get_name: usize,
        _get_path: usize,
        _get_folder: usize,
        _get_folders: usize,
        _create_folder: usize,
        _delete_folder: usize,
        get_task: unsafe extern "system" fn(*mut Obj, Bstr, *mut *mut Obj) -> i32,
        _get_tasks: usize,
        delete_task: unsafe extern "system" fn(*mut Obj, Bstr, i32) -> i32,
        #[allow(clippy::type_complexity)]
        register_task: unsafe extern "system" fn(*mut Obj, Bstr, Bstr, i32, Variant, Variant, i32, Variant, *mut *mut Obj) -> i32,
    }

    #[repr(C)]
    struct TaskVtbl {
        _base: Dispatch,
        _get_name: usize,
        _get_path: usize,
        _get_state: usize,
        _get_enabled: usize,
        _put_enabled: usize,
        run: unsafe extern "system" fn(*mut Obj, Variant, *mut *mut Obj) -> i32,
        _run_ex: usize,
        _get_instances: usize,
        _get_last_run_time: usize,
        _get_last_task_result: usize,
        _get_number_of_missed_runs: usize,
        _get_next_run_time: usize,
        _get_definition: usize,
        get_xml: unsafe extern "system" fn(*mut Obj, *mut Bstr) -> i32,
    }

    // WTSINFOW: the session, its logon among the rest.
    #[repr(C)]
    struct SessionInfo {
        _state: i32,
        _session_id: u32,
        _traffic: [u32; 6],
        _station: [u16; 32],
        _domain: [u16; 17],
        _user: [u16; 21],
        _connect_time: i64,
        _disconnect_time: i64,
        _last_input_time: i64,
        logon_time: i64,
        _current_time: i64,
    }

    #[link(name = "oleaut32")]
    extern "system" {
        fn SysAllocStringLen(text: *const u16, len: u32) -> Bstr;
        fn SysFreeString(text: Bstr);
        fn SysStringLen(text: Bstr) -> u32;
    }

    #[link(name = "advapi32")]
    extern "system" {
        fn ConvertSidToStringSidW(sid: *mut c_void, text: *mut *mut u16) -> i32;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetSystemTimeAsFileTime(time: *mut i64);
    }

    #[link(name = "wtsapi32")]
    extern "system" {
        fn WTSQuerySessionInformationW(server: isize, session: u32, class: i32, buffer: *mut *mut c_void, bytes: *mut u32) -> i32;
        fn WTSFreeMemory(memory: *mut c_void);
    }

    // A BSTR of ours, freed once let go.
    struct Text(Bstr);

    impl Text {
        fn new(text: &str) -> Self {
            let wide: Vec<u16> = text.encode_utf16().collect();
            // SAFETY: copies the given length from our own buffer.
            Text(unsafe { SysAllocStringLen(wide.as_ptr(), wide.len() as u32) })
        }

        fn read(&self) -> String {
            if self.0.is_null() {
                return String::new();
            }
            // SAFETY: a BSTR is as long as its length says.
            unsafe { String::from_utf16_lossy(std::slice::from_raw_parts(self.0, SysStringLen(self.0) as usize)) }
        }
    }

    impl Drop for Text {
        fn drop(&mut self) {
            // SAFETY: ours, or one given to us to free; null is allowed.
            unsafe { SysFreeString(self.0) };
        }
    }

    // One of the Task Scheduler's objects, released once let go.
    struct Held(*mut Obj);

    impl Held {
        // SAFETY (the caller's): V is this object's table.
        unsafe fn table<V>(&self) -> &V {
            &*((*self.0).vtbl as *const V)
        }
    }

    impl Drop for Held {
        fn drop(&mut self) {
            if !self.0.is_null() {
                // SAFETY: ours, released once.
                unsafe { (self.table::<Dispatch>().release)(self.0) };
            }
        }
    }

    // COM up on this thread while it lives (undone only if this did it).
    struct Apartment(bool);

    impl Drop for Apartment {
        fn drop(&mut self) {
            if self.0 {
                // SAFETY: the initialization this undoes.
                unsafe { CoUninitialize() };
            }
        }
    }

    fn check(what: &str, hr: i32) -> Result<(), String> {
        if hr < 0 {
            Err(format!("{what}: 0x{:08X}", hr as u32))
        } else {
            Ok(())
        }
    }

    // The Task Scheduler's root folder, for `f`. On a thread of its own, its
    // own COM's (whoever calls may have theirs, of another kind).
    fn in_root<T: Send>(f: impl FnOnce(&Held) -> Result<T, String> + Send) -> Result<T, String> {
        std::thread::scope(|s| {
            s.spawn(|| {
                // SAFETY: balanced by the apartment's drop, after every object here is released.
                let _apartment = Apartment(unsafe { CoInitializeEx(null_mut(), COINIT_MULTITHREADED) } >= 0);
                let mut service = Held(null_mut());
                // SAFETY: each out-parameter ours, each object released once used (Held).
                unsafe {
                    check("the Task Scheduler", CoCreateInstance(&CLSID_TASK_SCHEDULER, null_mut(), CLSCTX_INPROC_SERVER, &IID_ITASK_SERVICE, &mut service.0 as *mut *mut Obj as *mut *mut c_void))?;
                    let sv = service.table::<ServiceVtbl>();
                    check("the Task Scheduler's service", (sv.connect)(service.0, EMPTY, EMPTY, EMPTY, EMPTY))?;
                    let root = Text::new("\\");
                    let mut folder = Held(null_mut());
                    check("the Task Scheduler's root folder", (sv.get_folder)(service.0, root.0, &mut folder.0))?;
                    f(&folder)
                }
            })
            .join()
            .unwrap_or_else(|_| Err("the Task Scheduler's thread failed".into()))
        })
    }

    // The task's XML; none when there is no such task.
    pub fn xml(name: &str) -> Result<Option<String>, String> {
        in_root(|root| {
            let name = Text::new(name);
            let mut task = Held(null_mut());
            // SAFETY: as in_root.
            unsafe {
                let hr = (root.table::<FolderVtbl>().get_task)(root.0, name.0, &mut task.0);
                if NOT_FOUND.contains(&(hr as u32)) {
                    return Ok(None);
                }
                check("the task", hr)?;
                let mut xml = Text(null_mut());
                check("the task's XML", (task.table::<TaskVtbl>().get_xml)(task.0, &mut xml.0))?;
                Ok(Some(xml.read()))
            }
        })
    }

    // Made, or made anew (the same name).
    pub fn register(name: &str, xml: &str) -> Result<(), String> {
        in_root(|root| {
            let (name, xml) = (Text::new(name), Text::new(xml));
            let mut task = Held(null_mut());
            // SAFETY: as in_root.
            unsafe { check("the task not made", (root.table::<FolderVtbl>().register_task)(root.0, name.0, xml.0, TASK_CREATE_OR_UPDATE, EMPTY, EMPTY, TASK_LOGON_INTERACTIVE_TOKEN, EMPTY, &mut task.0)) }
        })
    }

    // Started now, as the Task Scheduler's Run does: with its own rights
    // (LibreHardwareMonitor's task, the highest: lhm_setup.rs), which its
    // author may ask for without being asked by Windows.
    pub fn run(name: &str) -> Result<(), String> {
        in_root(|root| {
            let name = Text::new(name);
            let mut task = Held(null_mut());
            // SAFETY: as in_root; the running task given back released (Held).
            unsafe {
                check("the task", (root.table::<FolderVtbl>().get_task)(root.0, name.0, &mut task.0))?;
                let mut running = Held(null_mut());
                check("the task not run", (task.table::<TaskVtbl>().run)(task.0, EMPTY, &mut running.0))
            }
        })
    }

    // Whether there was one to remove.
    pub fn remove(name: &str) -> Result<bool, String> {
        in_root(|root| {
            let name = Text::new(name);
            // SAFETY: as in_root.
            let hr = unsafe { (root.table::<FolderVtbl>().delete_task)(root.0, name.0, 0) };
            if NOT_FOUND.contains(&(hr as u32)) {
                return Ok(false);
            }
            check("the task not removed", hr).map(|_| true)
        })
    }

    // This user's SID (S-1-5-21-…), from the process's token.
    pub fn user_sid() -> Option<String> {
        // SAFETY: a token we close; TOKEN_USER written into our own aligned
        // buffer (a SID is at most 68 bytes, after a pointer and a DWORD);
        // the text Windows made, freed once read.
        unsafe {
            let mut token = 0isize;
            if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
                return None;
            }
            let mut buf = [0usize; 64];
            let mut needed = 0u32;
            let got = GetTokenInformation(token, TOKEN_USER, buf.as_mut_ptr().cast(), std::mem::size_of_val(&buf) as u32, &mut needed) != 0;
            CloseHandle(token);
            if !got || buf[0] == 0 {
                return None;
            }
            let mut text: *mut u16 = null_mut();
            if ConvertSidToStringSidW(buf[0] as *mut c_void, &mut text) == 0 || text.is_null() {
                return None;
            }
            let len = (0..).take_while(|&i| *text.add(i) != 0).count();
            let sid = String::from_utf16_lossy(std::slice::from_raw_parts(text, len));
            LocalFree(text.cast());
            Some(sid)
        }
    }

    // Seconds since this session's logon (Remote Desktop Services' record of
    // it, which any user may read of their own session).
    pub fn since_logon() -> Option<f64> {
        let mut buf: *mut c_void = null_mut();
        let mut bytes = 0u32;
        // SAFETY: Windows' buffer, read as WTSINFOW only when it is as long, then freed.
        let logon = unsafe {
            if WTSQuerySessionInformationW(0, WTS_CURRENT_SESSION, WTS_SESSION_INFO, &mut buf, &mut bytes) == 0 || buf.is_null() {
                return None;
            }
            let logon = (bytes as usize >= std::mem::size_of::<SessionInfo>()).then(|| (*(buf as *const SessionInfo)).logon_time);
            WTSFreeMemory(buf);
            logon?
        };
        let mut now = 0i64;
        // SAFETY: writes the one FILETIME.
        unsafe { GetSystemTimeAsFileTime(&mut now) };
        (logon > 0 && now >= logon).then(|| (now - logon) as f64 / 1e7)
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn xml(_name: &str) -> Result<Option<String>, String> {
        Err("not on this system".into())
    }

    pub fn register(_name: &str, _xml: &str) -> Result<(), String> {
        Err("not on this system".into())
    }

    pub fn remove(_name: &str) -> Result<bool, String> {
        Ok(false)
    }

    pub fn run(_name: &str) -> Result<(), String> {
        Err("not on this system".into())
    }

    pub fn user_sid() -> Option<String> {
        None
    }

    pub fn since_logon() -> Option<f64> {
        None
    }
}

pub use imp::{register, remove, run, since_logon, user_sid, xml};

// --- Start at login: this program, from where it is now -------------------------------
//
// The switch is the Run value (HKCU ...\CurrentVersion\Run, Wakuwaku): this
// program with AT_LOGIN_FLAG and "run". Task Manager's Startup apps list it
// and may turn it off there (StartupApproved\Run, by the same name), which
// is off here too. With it a logon task (logon.rs, with "task"), whatever
// her home: Windows starts the Run items one by one once Explorer is up,
// late and held back (three minutes after a logon on the user's laptop,
// zhoupengjie/Wakuwaku#44), while the task starts her as the user logs on.
// The Run value stays for when the task does not run; whichever of the two
// comes second finds her up and leaves without a word (any other start
// brings her back into sight).
//
// Kept to where she is: as she starts on her own port (not a test's copy on
// another, nor one with a data folder of its own), a Run value naming
// another exe (gone: renamed or moved by hand, a download kept under its
// own name; or another copy of her) is written anew for this one, and the
// task made, repaired or removed to match. Never turned on where it was off.
// Run from the source tree (cargo, its target folder), she writes no build's
// path over another exe's; her own value she keeps up.

pub const AT_LOGIN_FLAG: &str = "--wakuwaku-at-login";
pub const BY_TASK: &str = "task";
const BY_RUN: &str = "run";

// Started at login: by her logon task (task) or the Run value (run); none
// when started any other way.
pub fn started_by(args: &[String]) -> Option<&'static str> {
    let at = args.iter().position(|a| a == AT_LOGIN_FLAG)?;
    Some(if args.get(at + 1).map(String::as_str) == Some(BY_TASK) { BY_TASK } else { BY_RUN })
}

// That, for her log: ", by her logon task", ", by the Run value", or nothing.
pub fn said_by(by: Option<&str>) -> &'static str {
    match by {
        Some(BY_TASK) => ", by her logon task",
        Some(_) => ", by the Run value",
        None => "",
    }
}

// Start at login's decisions, plain (tested below; acted on, on Windows alone).
#[cfg_attr(not(windows), allow(dead_code))]
mod rules {
    use super::{AT_LOGIN_FLAG, BY_RUN, BY_TASK};
    use crate::agents::connection::same_path;
    use crate::logon::TaskSeen;

    // The Run value for this program, and what its logon task gives it.
    pub fn run_command(exe: &str) -> String {
        format!("\"{exe}\" {AT_LOGIN_FLAG} {BY_RUN}")
    }

    pub fn task_args() -> String {
        format!("{AT_LOGIN_FLAG} {BY_TASK}")
    }

    // The program a command line starts: its first word, in quotes or not
    // (a path with spaces and no quotes, up to its .exe).
    pub fn program_of(command: &str) -> &str {
        let command = command.trim();
        if let Some(rest) = command.strip_prefix('"') {
            return rest.split('"').next().unwrap_or("");
        }
        match command.to_ascii_lowercase().find(".exe") {
            Some(at) => &command[..at + 4],
            None => command.split_whitespace().next().unwrap_or(""),
        }
    }

    // Start at login as it stands: off (no Run value, or turned off in Task
    // Manager), on for this program, or on for another exe.
    #[derive(Debug, PartialEq, Clone, Copy)]
    pub enum Login {
        Off,
        Mine,
        Theirs,
    }

    pub fn login_of(value: Option<&str>, turned_off: bool, exe: &str) -> Login {
        match value {
            None => Login::Off,
            Some(_) if turned_off => Login::Off,
            Some(v) if same_path(program_of(v)) == same_path(exe) => Login::Mine,
            Some(_) => Login::Theirs,
        }
    }

    // What her start does with the Run value: keeps it, or writes this
    // program's in its place, and why (the exe it named is gone, it is
    // another copy of her, or it is this one as an older version wrote it).
    // No value stays none: start at login turned off stays off. Run from
    // the source tree, she takes no other exe's.
    #[derive(Debug, PartialEq)]
    pub enum RunFix {
        Keep,
        Rewrite(&'static str),
    }

    pub fn run_fix(value: Option<&str>, exe: &str, from_source: bool, exists: impl Fn(&str) -> bool) -> RunFix {
        let Some(value) = value else { return RunFix::Keep };
        let was = program_of(value);
        if same_path(was) == same_path(exe) {
            return if same_path(value) == same_path(&run_command(exe)) { RunFix::Keep } else { RunFix::Rewrite("as an older version wrote it") };
        }
        if from_source {
            return RunFix::Keep;
        }
        RunFix::Rewrite(if exists(was) { "another copy of her" } else { "gone" })
    }

    // What becomes of her logon task: there while start at login is hers,
    // whatever her home, running this program; gone otherwise. It was the
    // taskbar's alone at first, the island left to the Run value; but
    // Explorer starts the Run values one by one after its own, each waited
    // on, some half a minute in: on the user's PC hers came 96 and 110 s
    // after the logon, and a session shut down 52 s in never got to it
    // (2026-10-13, 10-15).
    #[derive(Debug, PartialEq)]
    pub enum TaskFix {
        Keep,
        Make,
        Remove,
    }

    pub fn task_fix(login: Login, task: Option<&TaskSeen>, exe: &str) -> TaskFix {
        match (login, task) {
            // Another exe's start at login: not this one's to change.
            (Login::Theirs, _) => TaskFix::Keep,
            (Login::Off, Some(_)) => TaskFix::Remove,
            (Login::Mine, None) => TaskFix::Make,
            // Turned off in the Task Scheduler: left as the person left it.
            (Login::Mine, Some(t)) if !t.enabled => TaskFix::Keep,
            (Login::Mine, Some(t)) if same_path(&t.program) != same_path(exe) || t.args != task_args() => TaskFix::Make,
            _ => TaskFix::Keep,
        }
    }
}

#[cfg(windows)]
mod login {
    use crate::agents::connection::launch_command;
    use super::rules::{login_of, run_command, run_fix, task_args, task_fix, Login, RunFix, TaskFix};
    use crate::logon;
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
    use winreg::RegKey;

    const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    // Task Manager's Startup apps: those turned off there, by name, their
    // first byte odd (03; turned on again, 02).
    const APPROVED: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";
    const NAME: &str = "Wakuwaku";

    fn value(name: &str) -> Option<String> {
        RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(RUN, KEY_READ).and_then(|k| k.get_value::<String, _>(name)).ok()
    }

    fn turned_off(name: &str) -> bool {
        RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags(APPROVED, KEY_READ)
            .and_then(|k| k.get_raw_value(name))
            .is_ok_and(|v| v.bytes.first().is_some_and(|b| b & 1 == 1))
    }

    fn write(name: &str, command: &str) -> Result<(), String> {
        let (key, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey_with_flags(RUN, KEY_READ | KEY_WRITE).map_err(|e| e.to_string())?;
        key.set_value(name, &command.to_string()).map_err(|e| e.to_string())
    }

    fn delete(path: &str, name: &str) -> Result<(), String> {
        let Ok(key) = RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(path, KEY_READ | KEY_WRITE) else { return Ok(()) };
        match key.delete_value(name) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        }
    }

    fn login(name: &str, exe: &str) -> Login {
        login_of(value(name).as_deref(), turned_off(name), exe)
    }

    // On for this program: a Run value naming it (written by an older
    // version or moved here as she started, too), not turned off in Task
    // Manager.
    pub fn is_on() -> bool {
        login(NAME, &launch_command()) == Login::Mine
    }

    // This user's logon task: the Run value's name, "at logon", their SID;
    // and the SID.
    fn task_of(name: &str) -> Result<(String, String), String> {
        let sid = logon::user_sid().ok_or("no SID for this user")?;
        Ok((format!("{name} at logon-{sid}"), sid))
    }

    // The logon task made, repaired or removed to match start at login;
    // what was done, if anything.
    fn sync_task(name: &str, exe: &str) -> Result<Option<String>, String> {
        let login = login(name, exe);
        if login == Login::Theirs {
            return Ok(None);
        }
        let (task, sid) = task_of(name)?;
        let seen = logon::xml(&task)?.map(|xml| logon::seen(&xml));
        match task_fix(login, seen.as_ref(), exe) {
            TaskFix::Keep => Ok(None),
            TaskFix::Make => {
                logon::register(&task, &logon::task_xml(&sid, exe, &task_args()))?;
                Ok(Some(match seen {
                    Some(was) => format!("her logon task pointed at {} {}; now {exe} {}", was.program, was.args, task_args()),
                    None => format!("her logon task made: {task}"),
                }))
            }
            TaskFix::Remove => {
                logon::remove(&task)?;
                Ok(Some("her logon task removed (start at login is off)".to_string()))
            }
        }
    }

    fn set_named(name: &str, exe: &str, on: bool) -> Result<(), String> {
        if on {
            write(name, &run_command(exe))?;
            // Turned off in Task Manager before: on there too, as asked here.
            delete(APPROVED, name)?;
        } else {
            delete(RUN, name)?;
        }
        sync_task(name, exe).map(drop).map_err(|e| format!("her logon task: {e}"))
    }

    // The switch: the Run value and the logon task, or neither.
    pub fn set(on: bool) -> Result<(), String> {
        set_named(NAME, &launch_command(), on)
    }

    fn at_start_named(name: &str, exe: &str, from_source: bool) -> Vec<String> {
        let mut said = Vec::new();
        let was = value(name);
        if let RunFix::Rewrite(why) = run_fix(was.as_deref(), exe, from_source, |p| std::path::Path::new(p).is_file()) {
            let now = run_command(exe);
            let was = was.unwrap_or_default();
            said.push(match write(name, &now) {
                Ok(()) => format!("start at login pointed at {was} ({why}); now {now}"),
                Err(e) => format!("start at login pointed at {was} ({why}); not written anew: {e}"),
            });
        }
        match sync_task(name, exe) {
            Ok(done) => said.extend(done),
            Err(e) => said.push(format!("her logon task: {e}")),
        }
        said
    }

    // As she starts on her own port: the Run value moved to this program,
    // the task to match; what was done, a line each.
    pub fn at_start() -> Vec<String> {
        at_start_named(NAME, &launch_command(), crate::data::from_source())
    }

    // Started by her logon task while start at login is off (the Run value
    // taken away, or turned off in Task Manager): the task goes, and true
    // (she goes too).
    pub fn task_unwanted() -> bool {
        if login(NAME, &launch_command()) != Login::Off {
            return false;
        }
        if let Ok((task, _)) = task_of(NAME) {
            let _ = logon::remove(&task);
        }
        true
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        // Against this computer's own registry and Task Scheduler, under a
        // test's name (WakuwakuTest, its task WakuwakuTest at logon-<SID>),
        // all of it taken away at the end: cargo test -- --ignored
        // start_at_login_kept_on_this_computer --nocapture.
        #[test]
        #[ignore]
        fn start_at_login_kept_on_this_computer() {
            const TEST: &str = "WakuwakuTest";
            let exe = launch_command();
            let (task, _) = task_of(TEST).unwrap();
            let was = r#""C:\nowhere\wakuwaku-v0.2.5-windows-x64.exe""#;
            let windows_view = || {
                let out = std::process::Command::new("schtasks").args(["/Query", "/TN", &task, "/V", "/FO", "LIST"]).output().unwrap();
                println!("{}", String::from_utf8_lossy(&out.stdout));
            };
            let clean = || {
                let _ = delete(RUN, TEST);
                let _ = delete(APPROVED, TEST);
                let _ = logon::remove(&task);
            };
            clean();

            // Off: nothing made at the start.
            assert!(at_start_named(TEST, &exe, false).is_empty());
            assert_eq!(value(TEST), None);
            assert_eq!(logon::xml(&task).unwrap(), None);

            // A value for an exe gone: this one's, and the task made for it.
            write(TEST, was).unwrap();
            let said = at_start_named(TEST, &exe, false);
            println!("{said:#?}");
            assert!(said[0].contains("(gone)"), "{said:?}");
            assert_eq!(value(TEST), Some(run_command(&exe)));
            assert_eq!(login(TEST, &exe), Login::Mine);
            let seen = logon::seen(&logon::xml(&task).unwrap().unwrap());
            assert_eq!((seen.program.as_str(), seen.args.as_str(), seen.enabled), (exe.as_str(), task_args().as_str(), true));
            windows_view();
            // Again: nothing to do.
            assert!(at_start_named(TEST, &exe, false).is_empty());

            // From the source tree another exe's value stays as it is.
            write(TEST, was).unwrap();
            assert!(at_start_named(TEST, &exe, true).is_empty());
            assert_eq!(value(TEST).as_deref(), Some(was));
            write(TEST, &run_command(&exe)).unwrap();

            // The task taken away in the Task Scheduler: made again.
            logon::remove(&task).unwrap();
            assert_eq!(sync_task(TEST, &exe).unwrap(), Some(format!("her logon task made: {task}")));

            // Turned off in Task Manager: off here, the task gone; on again from the switch.
            let off = winreg::RegValue { bytes: vec![3, 0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8].into(), vtype: winreg::enums::RegType::REG_BINARY };
            RegKey::predef(HKEY_CURRENT_USER).create_subkey(APPROVED).unwrap().0.set_raw_value(TEST, &off).unwrap();
            assert_eq!(login(TEST, &exe), Login::Off);
            assert_eq!(at_start_named(TEST, &exe, false), vec!["her logon task removed (start at login is off)".to_string()]);
            set_named(TEST, &exe, true).unwrap();
            assert!(!turned_off(TEST));
            assert_eq!(login(TEST, &exe), Login::Mine);
            assert!(logon::xml(&task).unwrap().is_some());

            // Off from the switch: both gone.
            set_named(TEST, &exe, false).unwrap();
            assert_eq!(value(TEST), None);
            assert_eq!(logon::xml(&task).unwrap(), None);
            clean();
        }
    }
}

#[cfg(not(windows))]
mod login {
    pub fn is_on() -> bool {
        false
    }

    pub fn set(_on: bool) -> Result<(), String> {
        Err("not on this system".into())
    }

    pub fn at_start() -> Vec<String> {
        Vec::new()
    }

    pub fn task_unwanted() -> bool {
        false
    }
}

pub use login::{at_start as login_at_start, is_on as is_open_at_login, set as set_open_at_login, task_unwanted as login_task_unwanted};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_task_reads_back_as_written() {
        let exe = r"D:\Program Files\张三 & co\wakuwaku.exe";
        let xml = task_xml("S-1-5-21-1-2-3-1001", exe, "--wakuwaku-at-login task");
        assert!(xml.contains("<UserId>S-1-5-21-1-2-3-1001</UserId>"));
        assert!(xml.contains(r"<Command>D:\Program Files\张三 &amp; co\wakuwaku.exe</Command>"));
        // Started at once, on battery too, never ended for running long, at normal priority.
        assert!(!xml.contains("<Delay>"));
        assert!(xml.contains("<DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>"));
        assert!(xml.contains("<StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>"));
        assert!(xml.contains("<ExecutionTimeLimit>PT0S</ExecutionTimeLimit>"));
        assert!(xml.contains("<Priority>4</Priority>"));
        assert_eq!(seen(&xml), TaskSeen { program: exe.into(), args: "--wakuwaku-at-login task".into(), enabled: true });
        // As the Task Scheduler may give it back: the program quoted, the task turned off there.
        let back = xml.replace(r"<Command>D:", r"<Command>&quot;D:").replace(r"wakuwaku.exe</Command>", r"wakuwaku.exe&quot;</Command>").replace("<Enabled>true</Enabled>\n  </Settings>", "<Enabled>false</Enabled>\n  </Settings>");
        assert_eq!(seen(&back), TaskSeen { program: exe.into(), args: "--wakuwaku-at-login task".into(), enabled: false });
        assert_eq!(seen("<Task><Actions><Exec><Command>C:\\w.exe</Command><Arguments/></Exec></Actions></Task>").args, "");
    }

    #[test]
    fn how_long_reads_short() {
        assert_eq!(how_long(3.24), "3.2 s");
        assert_eq!(how_long(-1.0), "0.0 s");
        assert_eq!(how_long(185.0), "3 min");
        assert_eq!(how_long(7.5 * 3600.0), "7 h 30 min");
    }

    // This session's logon, read as any user reads it.
    #[cfg(windows)]
    #[test]
    fn this_sessions_logon_reads() {
        let since = since_logon().expect("the session's logon");
        println!("logged on {} ago", how_long(since));
        assert!(since > 0.0 && since < 400.0 * 24.0 * 3600.0, "{since}");
        assert!(user_sid().is_some_and(|sid| sid.starts_with("S-1-5-")));
    }

    #[test]
    fn start_at_login_moves_with_her() {
        use super::rules::*;
        use crate::logon::TaskSeen;
        let exe = r"D:\Program Files\Wakuwaku\wakuwaku.exe";
        let ours = run_command(exe);
        assert_eq!(ours, r#""D:\Program Files\Wakuwaku\wakuwaku.exe" --wakuwaku-at-login run"#);
        let old = r#""D:\Program Files\Wakuwaku\wakuwaku-v0.2.5-windows-x64.exe""#;
        let before = format!("\"{exe}\"");
        let (gone, there) = (|_: &str| false, |_: &str| true);

        // Off stays off, whatever she is.
        assert_eq!(run_fix(None, exe, false, gone), RunFix::Keep);
        // Renamed, moved, or another copy: this one's in its place; from a
        // source tree no other exe's.
        assert_eq!(run_fix(Some(old), exe, false, gone), RunFix::Rewrite("gone"));
        assert_eq!(run_fix(Some(old), exe, false, there), RunFix::Rewrite("another copy of her"));
        assert_eq!(run_fix(Some(old), exe, true, gone), RunFix::Keep);
        // Hers: as this version writes it, or as 0.3.0 did (no flag), which
        // even a source tree's run brings up to date.
        assert_eq!(run_fix(Some(&ours), exe, false, there), RunFix::Keep);
        assert_eq!(run_fix(Some(&before), exe, true, there), RunFix::Rewrite("as an older version wrote it"));
        if cfg!(windows) {
            assert_eq!(run_fix(Some(&ours.to_uppercase()), exe, false, there), RunFix::Keep);
        }

        // The program a value starts, quoted or not.
        assert_eq!(program_of(&ours), exe);
        assert_eq!(program_of(r"C:\Program Files\W\w.EXE --x"), r"C:\Program Files\W\w.EXE");
        assert_eq!(program_of(r"C:\w\w --x"), r"C:\w\w");
        assert_eq!(program_of(""), "");

        // On: a value naming this program, in either form, not turned off in Task Manager.
        assert_eq!(login_of(None, false, exe), Login::Off);
        assert_eq!(login_of(Some(&ours), true, exe), Login::Off);
        assert_eq!(login_of(Some(&ours), false, exe), Login::Mine);
        assert_eq!(login_of(Some(&before), false, exe), Login::Mine);
        assert_eq!(login_of(Some(old), false, exe), Login::Theirs);

        // The logon task: there whenever start at login is hers (the island
        // her home too), running this program; left alone when turned off
        // in the Task Scheduler or another exe's.
        let task = |program: &str, enabled| TaskSeen { program: program.into(), args: task_args(), enabled };
        let elsewhere = r"D:\old\wakuwaku.exe";
        assert_eq!(task_fix(Login::Mine, None, exe), TaskFix::Make);
        assert_eq!(task_fix(Login::Mine, Some(&task(exe, true)), exe), TaskFix::Keep);
        assert_eq!(task_fix(Login::Mine, Some(&task(elsewhere, true)), exe), TaskFix::Make);
        assert_eq!(task_fix(Login::Mine, Some(&TaskSeen { args: String::new(), ..task(exe, true) }), exe), TaskFix::Make);
        assert_eq!(task_fix(Login::Mine, Some(&task(elsewhere, false)), exe), TaskFix::Keep);
        assert_eq!(task_fix(Login::Off, Some(&task(exe, false)), exe), TaskFix::Remove);
        assert_eq!(task_fix(Login::Off, Some(&task(exe, true)), exe), TaskFix::Remove);
        assert_eq!(task_fix(Login::Off, None, exe), TaskFix::Keep);
        assert_eq!(task_fix(Login::Theirs, Some(&task(elsewhere, true)), exe), TaskFix::Keep);
        assert_eq!(task_fix(Login::Theirs, None, exe), TaskFix::Keep);

        // How she was started, as the two give it to her.
        let args = |line: &str| line.split(' ').map(String::from).collect::<Vec<_>>();
        assert_eq!(started_by(&args("wakuwaku.exe")), None);
        assert_eq!(started_by(&args(&format!("wakuwaku.exe {}", &ours[ours.find(" --").unwrap() + 1..]))), Some("run"));
        assert_eq!(started_by(&args(&format!("wakuwaku.exe {} --wakuwaku-out-of-job", task_args()))), Some("task"));
        // And what her log says of it.
        assert_eq!(said_by(started_by(&args(&format!("wakuwaku.exe {}", task_args())))), ", by her logon task");
        assert_eq!(said_by(Some(BY_RUN)), ", by the Run value");
        assert_eq!(said_by(None), "");
    }
}
