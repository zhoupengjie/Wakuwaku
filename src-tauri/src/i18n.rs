// The few words the Rust side says itself (the menu, the tray's tooltip),
// from src/shared/i18n.js; the page has the rest.

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

pub fn t(lang: &str, key: &str) -> String {
    let zh = lang == "zh";
    let s = match key {
        "menu.pet" => if zh { "宠物" } else { "Pet" },
        "menu.noPets" => if zh { "还没有宠物" } else { "No pets yet" },
        "menu.size" => if zh { "大小" } else { "Size" },
        "menu.small" => if zh { "小" } else { "Small" },
        "menu.medium" => if zh { "中" } else { "Medium" },
        "menu.large" => if zh { "大" } else { "Large" },
        "menu.bubble" => if zh { "显示气泡" } else { "Show bubble" },
        "menu.walk" => if zh { "空闲时走动" } else { "Walk around when idle" },
        "menu.look" => if zh { "眼睛跟着鼠标" } else { "Follow the mouse" },
        "menu.backToCorner" => if zh { "回到右下角" } else { "Back to the corner" },
        "menu.home" => if zh { "她的家" } else { "Her home" },
        "menu.corner" => if zh { "角落头像" } else { "A corner" },
        "menu.bar" => if zh { "顶栏" } else { "The top bar" },
        "menu.taskbar" => if zh { "任务栏（代替 Windows 的）" } else { "The taskbar (in place of Windows')" },
        "menu.dnd" => if zh { "勿扰" } else { "Do not disturb" },
        "menu.show" => if zh { "显示宠物" } else { "Show pet" },
        "menu.hide" => if zh { "隐藏宠物" } else { "Hide pet" },
        "menu.island" => if zh { "灵动岛" } else { "The island" },
        "menu.letOut" => if zh { "放她出来" } else { "Let her out" },
        "menu.callBack" => if zh { "叫她回家" } else { "Call her back home" },
        "menu.settings" => if zh { "设置…" } else { "Settings…" },
        "menu.winSettings" => if zh { "Windows 设置" } else { "Windows Settings" },
        "menu.controlPanel" => if zh { "控制面板" } else { "Control Panel" },
        "menu.deviceManager" => if zh { "设备管理器" } else { "Device Manager" },
        "menu.quit" => if zh { "退出" } else { "Quit" },
        "deny.message" => if zh { "用户在桌宠上拒绝了。" } else { "The user denied this on the desktop pet." },
        "codex.waiting" => if zh { "等你在 Wakuwaku 上确认…" } else { "Waiting for your answer on Wakuwaku…" },
        "notify.waiting" => if zh { "{project}需要你确认" } else { "{project}needs your OK" },
        "notify.done" => if zh { "{project}做完了" } else { "{project}is done" },
        "notify.review" => if zh { "{project}改好了，等你看看" } else { "{project}has changes for you to review" },
        "notify.error" => if zh { "{project}出错了" } else { "{project}hit an error" },
        "notify.project" => if zh { "{project}：" } else { "{project}: " },
        "mail.new" => if zh { "新邮件 · {what}" } else { "New mail · {what}" },
        "mail.talkDone" => if zh { "{agent} 看完了：{what}" } else { "{agent} read it: {what}" },
        "mail.talkReplied" => if zh { "{agent} 回答了：{what}" } else { "{agent} answered: {what}" },
        "mail.handFailed" => if zh { "{agent} 没看成：{what}" } else { "{agent} could not read it: {what}" },
        "mood.idle" => if zh { "摸鱼中" } else { "Taking a break" },
        "mood.working" => if zh { "干活中…" } else { "Working…" },
        "mood.waiting" => if zh { "等你回复！" } else { "Waiting for you!" },
        "mood.done" => if zh { "搞定啦 ✓" } else { "All done ✓" },
        "mood.review" => if zh { "改好了，来看看 ✓" } else { "Changes ready for review ✓" },
        "mood.error" => if zh { "呜…出错了" } else { "Oops, something went wrong" },
        _ => key,
    };
    s.to_string()
}
