// oh-my-pi (omp): an extension in its agent folder
//
// omp runs no hooks: what extends it is a module it loads (in its own Bun,
// inside omp.exe) from <agent dir>/extensions/ as it starts. Ours,
// integrations/omp/wakuwaku.ts, is written there with this copy's port, exe
// and data folder filled in (an omp started after that loads it), and taken
// out again; nothing else of omp's is touched. It reports to her port from
// inside omp (events/others.rs omp). The agent folder as omp 18.11.1 finds it (its
// utils/src/dirs.ts): ~/.omp/agent, PI_CONFIG_DIR in place of .omp,
// PI_CODING_AGENT_DIR in place of the whole; and a profile (omp --profile
// <name>, or OMP_PROFILE) has one of its own, ~/.omp/profiles/<name>/agent,
// which she cannot tell from here: the extension goes into each one there is.

use super::*;

pub const OMP_FILE: &str = "wakuwaku.ts";
const OMP_SOURCE: &str = include_str!("../../../../integrations/omp/wakuwaku.ts");

pub(super) fn env_dir(name: &str) -> Option<PathBuf> {
    std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from)
}

// omp's own folder: ~/.omp, or what PI_CONFIG_DIR names under the home folder.
pub fn omp_root() -> PathBuf {
    home_dir().join(env_dir("PI_CONFIG_DIR").unwrap_or_else(|| PathBuf::from(".omp")))
}

// Its agent folders: the one it uses with no profile, then each profile's.
pub fn omp_agent_dirs() -> Vec<PathBuf> {
    agent_dirs_in(&omp_root(), env_dir("PI_CODING_AGENT_DIR"))
}

fn agent_dirs_in(root: &Path, agent_dir: Option<PathBuf>) -> Vec<PathBuf> {
    let mut dirs = vec![agent_dir.unwrap_or_else(|| root.join("agent"))];
    let mut profiles: Vec<PathBuf> = fs::read_dir(root.join("profiles"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path().join("agent"))
        .filter(|agent| agent.is_dir())
        .collect();
    profiles.sort();
    for dir in profiles {
        if !dirs.iter().any(|d| same_path(&d.to_string_lossy()) == same_path(&dir.to_string_lossy())) {
            dirs.push(dir);
        }
    }
    dirs
}

pub fn omp_files() -> Vec<PathBuf> {
    omp_agent_dirs().into_iter().map(|dir| dir.join("extensions").join(OMP_FILE)).collect()
}

// Whether omp is on this computer: its folder (made as it first runs), or
// its exe where its installer puts it.
pub fn omp_here() -> bool {
    let installed = env_dir("LOCALAPPDATA").is_some_and(|dir| dir.join("omp").join("omp.exe").is_file());
    omp_root().is_dir() || omp_agent_dirs()[0].is_dir() || installed
}

// The extension as a copy of her on this port, this exe and this data
// folder ("" for her own) writes it.
pub fn omp_extension(port: u16, exe: &str, data: &str) -> String {
    OMP_SOURCE
        .replace("\"__WAKUWAKU_PORT__\"", &format!("\"{port}\""))
        .replace("\"__WAKUWAKU_EXE__\"", &json!(exe).to_string())
        .replace("\"__WAKUWAKU_DATA__\"", &json!(data).to_string())
}

// What this copy writes: her data folder only when she was given one
// (WAKUWAKU_USER_DATA, a test's copy), for the one the extension starts.
fn omp_wanted(port: u16) -> String {
    omp_extension(port, &launch_command(), &std::env::var("WAKUWAKU_USER_DATA").unwrap_or_default())
}

// How the files stand against what this copy writes (None: no file in that
// folder): missing (in none) / ok / stale (another port or copy, an older
// version) / partial (a profile made since lacks it).
pub fn omp_status_of(found: &[Option<String>], want: &str) -> &'static str {
    if found.iter().all(Option::is_none) {
        "missing"
    } else if found.iter().flatten().any(|text| text != want) {
        "stale"
    } else if found.iter().all(Option::is_some) {
        "ok"
    } else {
        "partial"
    }
}

// absent (no omp here), unreadable, or how the extension stands.
pub fn omp_hooks_status(port: u16) -> &'static str {
    if !omp_here() {
        return "absent";
    }
    let mut found = Vec::new();
    for file in omp_files() {
        match fs::read_to_string(&file) {
            Ok(text) => found.push(Some(text)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => found.push(None),
            Err(_) => return "unreadable",
        }
    }
    omp_status_of(&found, &omp_wanted(port))
}

// Write the extension into every agent folder, or take it out of each (its
// extensions folder too, once empty). action: install, remove.
pub fn write_omp_hooks(action: &str, port: u16) -> Result<(), String> {
    write_omp_files(&omp_files(), action, &omp_wanted(port))
}

fn write_omp_files(files: &[PathBuf], action: &str, want: &str) -> Result<(), String> {
    match action {
        "install" => {
            for file in files {
                if let Some(dir) = file.parent() {
                    fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
                }
                fs::write(file, want).map_err(|e| format!("{}: {e}", file.display()))?;
            }
            Ok(())
        }
        "remove" => {
            for file in files {
                match fs::remove_file(file) {
                    Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(format!("{}: {e}", file.display())),
                    _ => {}
                }
                if let Some(dir) = file.parent() {
                    let _ = fs::remove_dir(dir);
                }
            }
            Ok(())
        }
        _ => Err(format!("unknown action {action}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_omp_extension_is_written_for_this_copy() {
        let exe = r#"C:\Program Files\Wakuwaku\it's "her".exe"#;
        let text = omp_extension(47312, exe, r"C:\waku\omp\data");
        assert!(text.contains(r#"const PORT = Number("47312") || 47213"#), "the port filled in");
        assert!(text.contains(&format!("const EXE = {}", json!(exe))), "the exe as a JS string");
        assert!(text.contains(r#"const DATA = "C:\\waku\\omp\\data""#));
        assert!(!text.contains("\"__WAKUWAKU_"), "nothing left to fill in");
        assert!(omp_extension(47213, "", "").contains(r#"const DATA = """#));
        // Every agent folder has it as this copy writes it, or not.
        let (ours, old) = (Some(text.clone()), Some(omp_extension(47213, exe, "")));
        assert_eq!(omp_status_of(&[None, None], &text), "missing");
        assert_eq!(omp_status_of(&[ours.clone()], &text), "ok");
        assert_eq!(omp_status_of(&[ours.clone(), ours.clone()], &text), "ok");
        assert_eq!(omp_status_of(&[ours.clone(), None], &text), "partial");
        assert_eq!(omp_status_of(&[ours.clone(), old.clone()], &text), "stale");
        assert_eq!(omp_status_of(&[None, old], &text), "stale");
    }

    #[test]
    fn the_omp_extension_goes_into_each_agent_folder_and_out() {
        let root = std::env::temp_dir().join(format!("wakuwaku-omp-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        // No profiles: the agent folder alone, made or not; PI_CODING_AGENT_DIR in its place.
        assert_eq!(agent_dirs_in(&root, None), vec![root.join("agent")]);
        assert_eq!(agent_dirs_in(&root, Some(PathBuf::from(r"D:\omp-agent"))), vec![PathBuf::from(r"D:\omp-agent")]);
        // Each profile's (a folder with no agent folder yet is none).
        for p in ["work", "home"] {
            fs::create_dir_all(root.join("profiles").join(p).join("agent")).unwrap();
        }
        fs::create_dir_all(root.join("profiles").join("empty")).unwrap();
        let dirs = agent_dirs_in(&root, None);
        assert_eq!(dirs, vec![root.join("agent"), root.join("profiles").join("home").join("agent"), root.join("profiles").join("work").join("agent")]);

        let files: Vec<PathBuf> = dirs.iter().map(|d| d.join("extensions").join(OMP_FILE)).collect();
        let theirs = root.join("agent").join("extensions").join("theirs.ts");
        fs::create_dir_all(theirs.parent().unwrap()).unwrap();
        fs::write(&theirs, "export default () => {}").unwrap();
        write_omp_files(&files, "install", "ours").unwrap();
        assert!(files.iter().all(|f| fs::read_to_string(f).unwrap() == "ours"));
        write_omp_files(&files, "install", "ours again").unwrap();
        assert_eq!(fs::read_to_string(&files[1]).unwrap(), "ours again");
        // Out: ours alone, and an extensions folder left empty.
        write_omp_files(&files, "remove", "").unwrap();
        assert!(files.iter().all(|f| !f.exists()));
        assert!(theirs.exists(), "another extension stays");
        assert!(!root.join("profiles").join("work").join("agent").join("extensions").exists());
        assert!(root.join("profiles").join("work").join("agent").is_dir());
        write_omp_files(&files, "remove", "").unwrap();
        assert!(write_omp_files(&files, "frob", "").is_err());
        let _ = fs::remove_dir_all(&root);
    }
}
