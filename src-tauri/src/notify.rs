// System notifications. Windows shows a toast only for an app it knows, so
// the first time this registers her id (HKCU\Software\Classes\AppUserModelId),
// with a name and an icon: nothing to install.
use std::path::Path;

pub const APP_ID: &str = "com.zhoupengjie.wakuwaku";

#[cfg(windows)]
pub fn register(dir: &Path) {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    let icon = dir.join("icon.png");
    if !icon.exists() {
        let _ = std::fs::create_dir_all(dir);
        let _ = std::fs::write(&icon, include_bytes!("../icons/icon.png"));
    }
    if let Ok((key, _)) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(format!(r"Software\Classes\AppUserModelId\{APP_ID}")) {
        let _ = key.set_value("DisplayName", &"Wakuwaku");
        let _ = key.set_value("IconUri", &icon.to_string_lossy().into_owned());
    }
}

#[cfg(windows)]
pub fn show(body: &str) -> Result<(), String> {
    tauri_winrt_notification::Toast::new(APP_ID)
        .title("Wakuwaku")
        .text1(body)
        .sound(None)
        .show()
        .map_err(|e| e.to_string())
}

#[cfg(not(windows))]
pub fn register(_dir: &Path) {}

#[cfg(not(windows))]
pub fn show(_body: &str) -> Result<(), String> {
    Err("not on this system".into())
}
