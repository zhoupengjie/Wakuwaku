// The agents' versions, as their files say them
//
// For the Agent page (src/pet/agents.ts), where one is cheap to know: the
// version a program file carries (Claude Code's own claude.exe, ZCode's and
// DeepSeek Harness's desktop apps), taken only when the file names that
// product (omp.exe, built by Bun, says Bun's own, 1.4.3, for omp 18.11), or
// its npm package's package.json (Claude Code's, OpenCode's), each where it
// installs by default. Never by running it (Codex's comes from `codex
// --version`, asked already: codex_version). None where no file says;
// Antigravity's agy and omp's carry none of their own. Read at most once a
// minute.

use super::*;
use crate::lock::Lock;

// Its version as its file says it ("0.2.0-rc.2", "2.1.296"): the
// FileVersion text's first word (Windows' own add a build's name), four
// parts cut to three (ZCode's 3.15.3.8237 is its 3.15.3).
pub fn tidy_version(text: &str) -> Option<String> {
    let text = text.trim_end_matches('\0').split_whitespace().next().unwrap_or("");
    if text.is_empty() || !text.starts_with(|c: char| c.is_ascii_digit()) || text.len() > 40 {
        return None;
    }
    let parts: Vec<&str> = text.split('.').collect();
    Some(if parts.len() == 4 && parts.iter().all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit())) { parts[..3].join(".") } else { text.to_string() })
}

// A program file's version, when its ProductName holds product.
#[cfg(windows)]
pub(super) fn file_version(path: &Path, product: &str) -> Option<String> {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "version")]
    extern "system" {
        fn GetFileVersionInfoSizeW(name: *const u16, handle: *mut u32) -> u32;
        fn GetFileVersionInfoW(name: *const u16, handle: u32, len: u32, data: *mut c_void) -> i32;
        fn VerQueryValueW(block: *const c_void, sub: *const u16, buffer: *mut *mut c_void, len: *mut u32) -> i32;
    }
    let wide = |s: &std::ffi::OsStr| s.encode_wide().chain(std::iter::once(0)).collect::<Vec<u16>>();
    let file = wide(path.as_os_str());
    // SAFETY: a buffer of the size the call asks for; the values read point
    // into it and are copied before it goes.
    unsafe {
        let size = GetFileVersionInfoSizeW(file.as_ptr(), std::ptr::null_mut());
        if size == 0 {
            return None;
        }
        let mut data = vec![0u8; size as usize];
        if GetFileVersionInfoW(file.as_ptr(), 0, size, data.as_mut_ptr() as *mut c_void) == 0 {
            return None;
        }
        let (mut at, mut len): (*mut c_void, u32) = (std::ptr::null_mut(), 0);
        let translation = wide(std::ffi::OsStr::new("\\VarFileInfo\\Translation"));
        if VerQueryValueW(data.as_ptr() as *const c_void, translation.as_ptr(), &mut at, &mut len) == 0 || len < 4 {
            return None;
        }
        let (lang, page) = (*(at as *const u16), *(at as *const u16).add(1));
        let text_of = |name: &str| {
            let key = wide(std::ffi::OsStr::new(&format!("\\StringFileInfo\\{lang:04x}{page:04x}\\{name}")));
            let (mut at, mut len): (*mut c_void, u32) = (std::ptr::null_mut(), 0);
            (VerQueryValueW(data.as_ptr() as *const c_void, key.as_ptr(), &mut at, &mut len) != 0 && len > 1)
                .then(|| String::from_utf16_lossy(std::slice::from_raw_parts(at as *const u16, len as usize)))
        };
        if !text_of("ProductName").is_some_and(|name| name.to_lowercase().contains(&product.to_lowercase())) {
            return None;
        }
        tidy_version(&text_of("FileVersion")?)
    }
}

#[cfg(not(windows))]
pub(super) fn file_version(_path: &Path, _product: &str) -> Option<String> {
    None
}

// An npm package's version, from its package.json.
pub(super) fn package_version(file: &Path) -> Option<String> {
    let text = fs::read_to_string(file).ok()?;
    let v: Value = serde_json::from_str(&text).ok()?;
    tidy_version(v.get("version")?.as_str()?)
}

pub fn versions() -> Value {
    static KNOWN: Lock<Option<(Instant, Value)>> = Lock::new("connection::versions::KNOWN", None);
    let mut known = KNOWN.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, v)) = known.as_ref().filter(|(at, _)| at.elapsed() < Duration::from_secs(60)) {
        return v.clone();
    }
    let local = env_dir("LOCALAPPDATA");
    let npm = env_dir("APPDATA").map(|a| a.join("npm").join("node_modules"));
    let in_local = |rest: &[&str]| local.as_ref().map(|l| rest.iter().fold(l.clone(), |p, part| p.join(part)));
    let in_npm = |rest: &[&str]| npm.as_ref().map(|n| rest.iter().fold(n.clone(), |p, part| p.join(part)).join("package.json"));
    let claude = file_version(&home_dir().join(".local").join("bin").join(if cfg!(windows) { "claude.exe" } else { "claude" }), "Claude Code")
        .or_else(|| in_npm(&["@anthropic-ai", "claude-code"]).and_then(|f| package_version(&f)));
    let v = json!({
        "claude": claude,
        // OpenCode 2 is @opencode/cli (2.0.29 in the test VM); before it, opencode-ai.
        "opencode": in_npm(&["@opencode", "cli"]).and_then(|f| package_version(&f)).or_else(|| in_npm(&["opencode-ai"]).and_then(|f| package_version(&f))),
        "zcode": in_local(&["Programs", "ZCode", "ZCode.exe"]).and_then(|f| file_version(&f, "ZCode")),
        "dsh": in_local(&["Programs", "DeepSeek Harness", "DeepSeek Harness.exe"]).and_then(|f| file_version(&f, "DeepSeek Harness")),
        "reasonix": reasonix_version(&in_local, &in_npm),
    });
    *known = Some((Instant::now(), v.clone()));
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_agents_version_as_its_file_says_it() {
        assert_eq!(tidy_version("2.1.296.0").as_deref(), Some("2.1.296"));
        assert_eq!(tidy_version("3.15.3.8237").as_deref(), Some("3.15.3"));
        assert_eq!(tidy_version("0.2.0-rc.2\0").as_deref(), Some("0.2.0-rc.2"));
        assert_eq!(tidy_version(" 2.0.29 ").as_deref(), Some("2.0.29"));
        // Windows' own: the build's name after it.
        assert_eq!(tidy_version("10.0.26100.1 (WinBuild.160101.0800)").as_deref(), Some("10.0.26100"));
        assert_eq!(tidy_version("1.2.3.beta").as_deref(), Some("1.2.3.beta"));
        assert_eq!(tidy_version(""), None);
        assert_eq!(tidy_version("unknown"), None);
        // An npm package's, from its package.json; none without one.
        let dir = std::env::temp_dir().join(format!("wakuwaku-version-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("package.json"), r#"{ "name": "opencode-ai", "version": "2.0.29" }"#).unwrap();
        assert_eq!(package_version(&dir.join("package.json")).as_deref(), Some("2.0.29"));
        assert_eq!(package_version(&dir.join("none.json")), None);
        fs::remove_dir_all(&dir).unwrap();
        // A program file's, read from it (every Windows has this one).
        #[cfg(windows)]
        {
            let kernel = PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("System32").join("kernel32.dll");
            assert!(file_version(&kernel, "Windows").is_some_and(|v| v.starts_with("10.") || v.starts_with("6.")), "{:?}", file_version(&kernel, "Windows"));
            // Another product's (as omp.exe says Bun): none.
            assert_eq!(file_version(&kernel, "Claude Code"), None);
        }
        assert_eq!(file_version(&std::env::temp_dir().join("no-such-program.exe"), "x"), None);
    }
}
