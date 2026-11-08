// The few words the Rust side says itself (the tray's menu and its tip,
// Windows' menus on the taskbar, her notices, dialogs and notifications),
// written where the pages' are, in src/shared/i18n.js, and nowhere else:
// build.rs reads the keys this program names from there into WORDS.

include!(concat!(env!("OUT_DIR"), "/words.rs"));

// 'zh' or 'en': as set, or as the system's language.
pub fn lang(setting: &str) -> &'static str {
    match setting {
        "zh" => "zh",
        "en" => "en",
        _ => match sys_locale::get_locale() {
            Some(locale) if locale.to_lowercase().starts_with("zh") => "zh",
            _ => "en",
        },
    }
}

// The words for key in lang ("zh", else English); the key itself where
// i18n.js has none, as the pages' t.
pub fn t(lang: &str, key: &str) -> String {
    match WORDS.binary_search_by(|(k, ..)| (*k).cmp(key)) {
        Ok(at) => {
            let (_, zh, en) = WORDS[at];
            if lang == "zh" { zh } else { en }.to_string()
        }
        Err(_) => key.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Every key the program asks for is in i18n.js and so in WORDS: the
    // ones it names at a t( call or in a list of them, and those it builds
    // (a mood's, a notification's, Windows' own menu items' on the taskbar:
    // tray.rs part_menu).
    #[test]
    fn every_word_the_program_asks_for_is_there() {
        let mut asked = Vec::new();
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut dirs = vec![src];
        while let Some(dir) = dirs.pop() {
            for entry in std::fs::read_dir(dir).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    dirs.push(path);
                    continue;
                }
                if path.extension().is_none_or(|x| x != "rs") || path.ends_with("i18n.rs") {
                    continue;
                }
                let code = std::fs::read_to_string(&path).unwrap();
                let is_key = |k: &&str| k.contains('.') && k.starts_with(|c: char| c.is_ascii_lowercase()) && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '.');
                for (at, _) in code.match_indices("t(") {
                    // t( itself, not assert( or format(.
                    if code[..at].ends_with(|c: char| c.is_ascii_alphanumeric() || c == '_') {
                        continue;
                    }
                    let call = &code[at + 2..code[at..].find(')').map_or(code.len(), |end| at + end)];
                    if let Some(key) = call.split('"').nth(1).filter(is_key) {
                        asked.push(key.to_string());
                    }
                }
            }
        }
        asked.extend(["taskbar.restoreAll", "taskbar.minimizeAll", "taskbar.closeAll"].map(String::from));
        for mood in ["idle", "working", "waiting", "done", "review", "error"] {
            asked.push(format!("mood.{mood}"));
        }
        for mood in ["waiting", "done", "review", "error"] {
            asked.push(format!("notify.{mood}"));
        }
        for item in ["soundFix", "mixer", "sound", "netFix", "network", "dateTime", "notifySettings"] {
            asked.push(format!("menu.{item}"));
        }
        assert!(asked.len() > 50, "found only {}: the scan is broken", asked.len());
        for key in asked {
            assert!(WORDS.binary_search_by(|(k, ..)| (*k).cmp(key.as_str())).is_ok(), "{key}: not in src/shared/i18n.js");
        }
    }

    #[test]
    fn words_in_either_language_or_the_key() {
        assert_eq!(t("zh", "menu.quit"), "退出应用");
        assert_eq!(t("en", "menu.quit"), "Quit app");
        assert_eq!(t("fr", "menu.quit"), "Quit app");
        assert_eq!(t("zh", "no.suchKey"), "no.suchKey");
        assert!(t("en", "crash.box").contains("{n} times in a row.

"));
    }
}
