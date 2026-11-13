// Easy on the battery and the fans. Two things Windows is told:
//
// - This process may run on the efficient cores at lower clocks (the
//   "efficiency mode" Task Manager shows with a leaf), while nobody is at
//   the keyboard or mouse (shared.rs check_quiet); and its background threads
//   always may (thread_eco), whatever the core layout: a CPU with small
//   cores (Intel's E-cores, AMD's dense cores) puts them there, one
//   without runs them slower. Windows keeps the choice itself; nothing is
//   pinned to a core. WebView2's processes for her pages too, while quiet
//   (webview_eco), and their memory kept low (webview_memory).
// - How long since the keyboard or mouse was last touched (idle_ms), for
//   the quiet state: she holds still and the loops look less often.
// - While someone is at it and no app is full screen (shared.rs keep_brisk),
//   her process and WebView2's for her pages before others' (brisk): above
//   normal on the CPU, high on the GPU (Windows' scheduling class, as OBS
//   sets its own), so her island, taskbar and dock keep up with the person
//   while something else has the CPU or the GPU busy (the user,
//   2026-11-08: GPU或者CPU高负载的情况下，Wakuwaku的整个界面很容易卡死). Never
//   high or realtime on the CPU: what makes the load would go wanting.

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;
    use crate::win32::{ProcessEntry, CloseHandle, CreateToolhelp32Snapshot, GetCurrentProcess, GetCurrentProcessId, GetTickCount, OpenProcess, Process32FirstW, Process32NextW};

    // PROCESS_POWER_THROTTLING_STATE and THREAD_POWER_THROTTLING_STATE: the
    // same three numbers.
    #[repr(C)]
    struct Throttling {
        version: u32,
        control: u32,
        state: u32,
    }

    const VERSION: u32 = 1;
    const EXECUTION_SPEED: u32 = 0x1;
    const PROCESS_POWER_THROTTLING: i32 = 4;
    // ThreadPowerThrottling, after ThreadMemoryPriority,
    // ThreadAbsoluteCpuPriority and ThreadDynamicCodePolicy (2: as that, the
    // call failed for its size, and no thread of hers was ever easy on the
    // power; 2026-10-13).
    const THREAD_POWER_THROTTLING: i32 = 3;

    #[repr(C)]
    struct LastInput {
        size: u32,
        time: u32,
    }

    // Handles as isize, as shared.rs declares GetCurrentProcess too.
    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentThread() -> isize;
        fn SetProcessInformation(process: isize, class: i32, info: *const c_void, size: u32) -> i32;
        fn GetProcessInformation(process: isize, class: i32, info: *mut c_void, size: u32) -> i32;
        fn SetThreadInformation(thread: isize, class: i32, info: *const c_void, size: u32) -> i32;
        fn SetThreadPriority(thread: isize, priority: i32) -> i32;
        fn SetPriorityClass(process: isize, class: u32) -> i32;
    }

    #[link(name = "gdi32")]
    extern "system" {
        // The GPU's scheduling class of a process (d3dkmthk.h): its work
        // before others' while the GPU is busy. An NTSTATUS, 0 done.
        fn D3DKMTSetProcessSchedulingPriorityClass(process: isize, class: i32) -> i32;
    }

    const NORMAL_PRIORITY_CLASS: u32 = 0x20;
    const ABOVE_NORMAL_PRIORITY_CLASS: u32 = 0x8000;
    // D3DKMT_SCHEDULINGPRIORITYCLASS_NORMAL and _HIGH (REALTIME, 5, asks
    // for the right to raise priorities, an administrator's).
    const GPU_NORMAL: i32 = 2;
    const GPU_HIGH: i32 = 4;

    const TH32CS_SNAPPROCESS: u32 = 0x2;
    const PROCESS_SET_INFORMATION: u32 = 0x0200;
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;

    #[link(name = "user32")]
    extern "system" {
        fn GetLastInputInfo(info: *mut LastInput) -> i32;
    }

    fn throttling(on: bool) -> Throttling {
        // Control 0 leaves it to Windows again.
        Throttling { version: VERSION, control: if on { EXECUTION_SPEED } else { 0 }, state: if on { EXECUTION_SPEED } else { 0 } }
    }

    // This process easy on the power, or back to Windows' own choice.
    pub fn process_eco(on: bool) -> bool {
        let state = throttling(on);
        // SAFETY: our own process; the struct is ours, its size passed.
        unsafe { SetProcessInformation(GetCurrentProcess(), PROCESS_POWER_THROTTLING, &state as *const Throttling as *const c_void, std::mem::size_of::<Throttling>() as u32) != 0 }
    }

    // WebView2's processes under this one: its browser process, and the
    // GPU, renderer and utility processes under that.
    fn webview_processes() -> Vec<u32> {
        let mut all = Vec::new();
        // SAFETY: a snapshot we close below; the entry is sized as the API asks.
        unsafe {
            let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snap == 0 || snap == -1 {
                return Vec::new();
            }
            let mut e: ProcessEntry = std::mem::zeroed();
            e.size = std::mem::size_of::<ProcessEntry>() as u32;
            let mut ok = Process32FirstW(snap, &mut e) != 0;
            while ok {
                let len = e.exe.iter().position(|&c| c == 0).unwrap_or(e.exe.len());
                all.push((e.pid, e.parent, String::from_utf16_lossy(&e.exe[..len]).eq_ignore_ascii_case("msedgewebview2.exe")));
                ok = Process32NextW(snap, &mut e) != 0;
            }
            CloseHandle(snap);
        }
        // SAFETY: a plain call.
        let mut found = vec![unsafe { GetCurrentProcessId() }];
        let mut i = 0;
        while i < found.len() {
            let parent = found[i];
            for &(pid, of, webview) in &all {
                if of == parent && webview && !found.contains(&pid) {
                    found.push(pid);
                }
            }
            i += 1;
        }
        found.split_off(1)
    }

    // WebView2's processes for her pages easy on the power too, or back to
    // their own choice (#14: hers alone was, and her pages' work is theirs).
    // Only the execution speed is ours to set: what else Chromium set is
    // kept (the timer resolution, for its GPU and renderer processes).
    pub fn webview_eco(on: bool) -> usize {
        let size = std::mem::size_of::<Throttling>() as u32;
        let mut done = 0;
        for pid in webview_processes() {
            // SAFETY: a handle we close; the struct is ours, its size passed.
            unsafe {
                let process = OpenProcess(PROCESS_SET_INFORMATION | PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
                if process == 0 {
                    continue;
                }
                let mut state = Throttling { version: VERSION, control: 0, state: 0 };
                if GetProcessInformation(process, PROCESS_POWER_THROTTLING, &mut state as *mut Throttling as *mut c_void, size) != 0 {
                    let speed = if on { EXECUTION_SPEED } else { 0 };
                    state.version = VERSION;
                    state.control = (state.control & !EXECUTION_SPEED) | speed;
                    state.state = (state.state & !EXECUTION_SPEED) | speed;
                    if SetProcessInformation(process, PROCESS_POWER_THROTTLING, &state as *const Throttling as *const c_void, size) != 0 {
                        done += 1;
                    }
                }
                CloseHandle(process);
            }
        }
        done
    }

    // Her process and WebView2's under it before others' on the CPU (above
    // normal) and the GPU (high), or back to normal. What was done, for the
    // log: how many of how many, and what the GPU's answer was if it refused.
    pub fn brisk(on: bool) -> String {
        let (cpu, gpu) = if on { (ABOVE_NORMAL_PRIORITY_CLASS, GPU_HIGH) } else { (NORMAL_PRIORITY_CLASS, GPU_NORMAL) };
        let (mut n, mut on_cpu, mut on_gpu, mut refused) = (0, 0, 0, 0i32);
        let mut set = |process: isize| {
            n += 1;
            // SAFETY: a process handle of ours (our own, or one opened with
            // the right to set its information); plain calls.
            unsafe {
                if SetPriorityClass(process, cpu) != 0 {
                    on_cpu += 1;
                }
                match D3DKMTSetProcessSchedulingPriorityClass(process, gpu) {
                    0 => on_gpu += 1,
                    status => refused = status,
                }
            }
        };
        // SAFETY: a pseudo handle to this process.
        set(unsafe { GetCurrentProcess() });
        for pid in webview_processes() {
            // SAFETY: a handle we close.
            unsafe {
                let process = OpenProcess(PROCESS_SET_INFORMATION | PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
                if process == 0 {
                    continue;
                }
                set(process);
                CloseHandle(process);
            }
        }
        let gpu_said = if refused != 0 { format!(" (refused: {refused:#x})") } else { String::new() };
        format!("{on_cpu} of {n} processes {} on the CPU, {on_gpu} {} on the GPU{gpu_said}", if on { "above normal" } else { "normal" }, if on { "high" } else { "normal" })
    }

    // Her pages' memory as WebView2 is to keep it: low while nobody is at
    // the computer (it may page some out), normal again as someone is; as
    // WebView2 has it done for an app gone inactive (#14).
    pub fn webview_memory(window: &tauri::WebviewWindow, low: bool) {
        use webview2_com::Microsoft::Web::WebView2::Win32::{ICoreWebView2_19, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL};
        use windows_core::Interface;
        let _ = window.with_webview(move |view| {
            // SAFETY: WebView2's own objects, on its thread, as it hands them.
            unsafe {
                if let Ok(core) = view.controller().CoreWebView2().and_then(|core| core.cast::<ICoreWebView2_19>()) {
                    let _ = core.SetMemoryUsageTargetLevel(if low { COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW } else { COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL });
                }
            }
        });
    }

    // The calling thread easy on the power, for good: a loop that only
    // looks at things now and then.
    pub fn thread_eco() -> bool {
        let state = throttling(true);
        // SAFETY: our own thread; the struct is ours, its size passed.
        unsafe { SetThreadInformation(GetCurrentThread(), THREAD_POWER_THROTTLING, &state as *const Throttling as *const c_void, std::mem::size_of::<Throttling>() as u32) != 0 }
    }

    // The calling thread quick, whatever the process is: the keys' hook
    // (winkeys.rs), which Windows waits on for every key and drops should
    // it answer late. Never slowed (its speed set, and set to none: the
    // process's efficiency mode, while quiet, is not its), and before the
    // others when it has something to do.
    pub fn thread_brisk() -> bool {
        const THREAD_PRIORITY_HIGHEST: i32 = 2;
        let state = Throttling { version: VERSION, control: EXECUTION_SPEED, state: 0 };
        // SAFETY: our own thread; the struct is ours, its size passed.
        unsafe {
            SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_HIGHEST);
            SetThreadInformation(GetCurrentThread(), THREAD_POWER_THROTTLING, &state as *const Throttling as *const c_void, std::mem::size_of::<Throttling>() as u32) != 0
        }
    }

    // Since the keyboard or mouse was last touched, ms; 0 when unknown.
    pub fn idle_ms() -> u64 {
        let mut last = LastInput { size: std::mem::size_of::<LastInput>() as u32, time: 0 };
        // SAFETY: our own struct, its size set.
        if unsafe { GetLastInputInfo(&mut last) } == 0 {
            return 0;
        }
        // SAFETY: a plain read.
        u64::from(unsafe { GetTickCount() }.wrapping_sub(last.time))
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn process_eco(_on: bool) -> bool {
        false
    }
    pub fn thread_eco() -> bool {
        false
    }
    pub fn thread_brisk() -> bool {
        false
    }
    pub fn webview_eco(_on: bool) -> usize {
        0
    }
    pub fn brisk(_on: bool) -> String {
        String::new()
    }
    pub fn webview_memory(_window: &tauri::WebviewWindow, _low: bool) {}
    pub fn idle_ms() -> u64 {
        0
    }
}

pub use imp::{brisk, idle_ms, process_eco, thread_brisk, thread_eco, webview_eco, webview_memory};
