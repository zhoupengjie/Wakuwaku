// OpenCode: our plugin in its plugin folder
//
// OpenCode 2 runs no hook commands: it loads JavaScript plugins, every .js in
// <its config folder>/plugins/ by itself (and again as one changes), into its
// background service. Ours is integrations/opencode/wakuwaku.js, written
// there with her port in it: it tells her port of its sessions and asks the
// person there too (events/others.rs opencode); no program of hers runs. Its config
// folder, as OpenCode 2.0.29 finds it: OPENCODE_CONFIG_DIR, else
// $XDG_CONFIG_HOME/opencode, else ~/.config/opencode (on Windows too).

use super::*;

const OPENCODE_PLUGIN: &str = include_str!("../../../../integrations/opencode/wakuwaku.js");
// The template's first line: ours, whatever port it was written with.
const OPENCODE_MARK: &str = "// wakuwaku-opencode-plugin";
const OPENCODE_PORT_LINE: &str = "const PORT = 47213";

pub(super) fn home_dir() -> PathBuf {
    PathBuf::from(std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).unwrap_or_default())
}

fn xdg(var: &str, under_home: &[&str]) -> PathBuf {
    std::env::var_os(var).filter(|v| !v.is_empty()).map(PathBuf::from).unwrap_or_else(|| under_home.iter().fold(home_dir(), |p, part| p.join(part)))
}

pub fn opencode_dir() -> PathBuf {
    std::env::var_os("OPENCODE_CONFIG_DIR").filter(|v| !v.is_empty()).map(PathBuf::from).unwrap_or_else(|| xdg("XDG_CONFIG_HOME", &[".config"]).join("opencode"))
}

pub fn opencode_plugin_file() -> PathBuf {
    opencode_dir().join("plugins").join("wakuwaku.js")
}

// Whether OpenCode is on this computer: its config folder, or its data
// folder (~/.local/share/opencode), which it makes as it first runs.
pub fn opencode_here() -> bool {
    opencode_dir().is_dir() || xdg("XDG_DATA_HOME", &[".local", "share"]).join("opencode").is_dir()
}

// The plugin as this copy writes it: the template, her port in it.
pub fn opencode_plugin(port: u16) -> String {
    OPENCODE_PLUGIN.replacen(OPENCODE_PORT_LINE, &format!("const PORT = {port}"), 1)
}

fn is_our_plugin(text: &str) -> bool {
    text.trim_start_matches('\u{feff}').starts_with(OPENCODE_MARK)
}

// How the file stands against what this copy would write: missing (none,
// or someone else's of that name) / ok / stale (another port, another
// version of ours). Line ends aside.
pub fn opencode_status_of(text: Option<&str>, port: u16) -> &'static str {
    match text {
        Some(text) if is_our_plugin(text) => {
            if text.replace("\r\n", "\n") == opencode_plugin(port).replace("\r\n", "\n") {
                "ok"
            } else {
                "stale"
            }
        }
        _ => "missing",
    }
}

// absent (no OpenCode here), unreadable, or how our plugin stands.
pub fn opencode_status(port: u16) -> &'static str {
    let file = opencode_plugin_file();
    match fs::read_to_string(&file) {
        Ok(text) => opencode_status_of(Some(&text), port),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            if opencode_here() {
                "missing"
            } else {
                "absent"
            }
        }
        Err(_) => "unreadable",
    }
}

// Write or take away our plugin. action: install, remove. Someone else's
// file of that name is kept (once) beside it before ours goes in its place,
// and never removed.
pub fn write_opencode_plugin(action: &str, port: u16) -> Result<(), String> {
    let file = opencode_plugin_file();
    let theirs = fs::read_to_string(&file).ok().filter(|t| !is_our_plugin(t));
    match action {
        "install" => {
            let backup = backup_of(&file);
            if theirs.is_some() && !backup.exists() {
                fs::copy(&file, &backup).map_err(|e| e.to_string())?;
            }
            if let Some(dir) = file.parent() {
                fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            }
            fs::write(&file, opencode_plugin(port)).map_err(|e| e.to_string())
        }
        "remove" if theirs.is_some() || !file.exists() => Ok(()),
        "remove" => {
            fs::remove_file(&file).map_err(|e| e.to_string())?;
            // Its plugins folder too, once nothing else is in it (as omp's).
            if let Some(dir) = file.parent() {
                let _ = fs::remove_dir(dir);
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
    fn the_opencode_plugin_carries_her_port() {
        // The template has its port once, on a line of its own.
        assert_eq!(OPENCODE_PLUGIN.matches(OPENCODE_PORT_LINE).count(), 1);
        let ours = opencode_plugin(47311);
        assert!(ours.contains("const PORT = 47311\n") || ours.contains("const PORT = 47311\r\n"));
        assert!(!ours.contains(OPENCODE_PORT_LINE));
        assert_eq!(opencode_status_of(Some(&ours), 47311), "ok");
        assert_eq!(opencode_status_of(Some(&ours.replace('\n', "\r\n").replace("\r\r", "\r")), 47311), "ok");
        // Another port, or another version of ours: a repair.
        assert_eq!(opencode_status_of(Some(&ours), 47213), "stale");
        assert_eq!(opencode_status_of(Some(&format!("{OPENCODE_MARK}\n// an older one\n")), 47311), "stale");
        // Someone else's file of that name, or none: not ours.
        assert_eq!(opencode_status_of(Some("export default { id: 'mine', setup() {} }"), 47311), "missing");
        assert_eq!(opencode_status_of(None, 47311), "missing");
    }

    #[test]
    fn opencode_s_folder_as_it_finds_it() {
        // Changing the process's environment: only here, and put back.
        let was = (std::env::var_os("OPENCODE_CONFIG_DIR"), std::env::var_os("XDG_CONFIG_HOME"));
        std::env::set_var("OPENCODE_CONFIG_DIR", "Z:\\oc-config");
        assert_eq!(opencode_plugin_file(), PathBuf::from("Z:\\oc-config").join("plugins").join("wakuwaku.js"));
        std::env::remove_var("OPENCODE_CONFIG_DIR");
        std::env::set_var("XDG_CONFIG_HOME", "Z:\\xdg");
        assert_eq!(opencode_dir(), PathBuf::from("Z:\\xdg").join("opencode"));
        std::env::remove_var("XDG_CONFIG_HOME");
        assert_eq!(opencode_dir(), home_dir().join(".config").join("opencode"));
        for (key, value) in [("OPENCODE_CONFIG_DIR", was.0), ("XDG_CONFIG_HOME", was.1)] {
            if let Some(value) = value {
                std::env::set_var(key, value);
            }
        }
    }
}
