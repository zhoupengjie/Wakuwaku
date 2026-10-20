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
        "menu.walk" => if zh { "空闲时走动" } else { "Walk around when idle" },
        "menu.look" => if zh { "眼睛跟着鼠标" } else { "Follow the mouse" },
        "menu.backToCorner" => if zh { "回到右下角" } else { "Back to the corner" },
        "menu.home" => if zh { "她的家" } else { "Her home" },
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
        "menu.taskManager" => if zh { "任务管理器" } else { "Task Manager" },
        "menu.editTaskbar" => if zh { "编辑任务栏布局" } else { "Edit the taskbar layout" },
        "menu.endEdit" => if zh { "退出编辑" } else { "Stop editing the taskbar" },
        // The widget picker and her capsule's panel (picker.js, capsule-panel.js).
        "menu.addWidgets" => if zh { "添加小工具…" } else { "Add widgets…" },
        "menu.editCapsule" => if zh { "编辑胶囊" } else { "Edit capsule" },
        "menu.deviceManager" => if zh { "设备管理器" } else { "Device Manager" },
        // Shift and a right press on a program's button with several windows (tasks.rs window_menu), in Windows' words.
        "taskbar.restoreAll" => if zh { "还原所有窗口" } else { "Restore all windows" },
        "taskbar.minimizeAll" => if zh { "最小化所有窗口" } else { "Minimize all windows" },
        "taskbar.closeAll" => if zh { "关闭所有窗口" } else { "Close all windows" },
        // Windows' own menus on the volume, the network and the clock, in
        // its words (tray.rs part_menu), and its clock's tip (clock.rs).
        "menu.soundFix" => if zh { "声音问题疑难解答" } else { "Troubleshoot sound problems" },
        "menu.mixer" => if zh { "打开音量合成器" } else { "Open volume mixer" },
        "menu.sound" => if zh { "声音设置" } else { "Sound settings" },
        "menu.netFix" => if zh { "诊断网络问题" } else { "Diagnose network problems" },
        "menu.network" => if zh { "网络和 Internet 设置" } else { "Network and Internet settings" },
        "menu.dateTime" => if zh { "调整日期和时间" } else { "Adjust date and time" },
        "menu.notifySettings" => if zh { "通知设置" } else { "Notifications settings" },
        "clock.local" => if zh { "本地时间" } else { "Local time" },
        // Another program fighting her over Windows' taskbar (taskbar.rs give_up, #69).
        "taskbar.tug" => if zh { "别的程序在管任务栏" } else { "Another app runs the taskbar" },
        "taskbar.tugTap" => if zh { "已切到灵动岛，点一下去外观设置" } else { "Switched to the island · tap for Appearance" },
        "update.out" =>if zh { "有新版本 v{version}" } else { "A new version: v{version}" },
        "update.tap" => if zh { "点一下去更新" } else { "Tap to update" },
        // An update whose new version was stopped or taken away as it started (updates.rs).
        "update.blocked" => if zh { "v{version} 没装上，多半是杀毒软件拦下了" } else { "v{version} was stopped, likely by an antivirus" },
        "update.blockedTap" => if zh { "点一下看怎么办" } else { "Tap to see what to do" },
        "crash.out" => if zh { "她上次意外退出了，要发崩溃报告吗？" } else { "She quit unexpectedly. Send a crash report?" },
        "crash.tap" => if zh { "点一下看看" } else { "Tap to see" },
        "crash.box" => if zh {
            "Wakuwaku 最近连续崩溃了 {n} 次。\n\n要从现在起自动发送崩溃报告吗？报告会公开发到 GitHub 或 Gitee 的 issue 里，用户名、路径、其他程序的名字和窗口标题、邮箱这类内容会先去掉。\n\n以后可以在 设置 → 通用 → 崩溃报告 里关掉。"
        } else {
            "Wakuwaku has crashed {n} times in a row.\n\nSend crash reports automatically from now on? They are posted publicly as issues on GitHub or Gitee, with user names, paths, other programs' names and window titles, mail addresses and the like taken out first.\n\nYou can turn this off in Settings → General → Crash reports."
        },
        "wallpaper.pickPicture" => if zh { "选择壁纸图片" } else { "Pick a picture for the desktop" },
        "wallpaper.pickFolder" => if zh { "选择一个文件夹，里面的图片轮流做壁纸" } else { "Pick a folder whose pictures take turns on the desktop" },
        "wallpaper.pictures" => if zh { "图片" } else { "Pictures" },
        "menu.quit" => if zh { "退出" } else { "Quit" },
        "deny.message" => if zh { "用户在桌宠上拒绝了。" } else { "The user denied this on the desktop pet." },
        "codex.waiting" => if zh { "等你在 Wakuwaku 上确认…" } else { "Waiting for your answer on Wakuwaku…" },
        "notify.waiting" => if zh { "{project}需要你确认" } else { "{project}needs your OK" },
        "notify.done" => if zh { "{project}做完了" } else { "{project}is done" },
        "notify.review" => if zh { "{project}改好了，等你看看" } else { "{project}has changes for you to review" },
        "notify.error" => if zh { "{project}出错了" } else { "{project}hit an error" },
        "notify.project" => if zh { "{project}：" } else { "{project}: " },
        // Words sent to her to say (say.rs), from no one named.
        "word.from" => if zh { "有话对你说" } else { "A word for you" },
        "mail.new" => if zh { "新邮件 · {what}" } else { "New mail · {what}" },
        "mail.talkDone" => if zh { "{agent} 看完了：{what}" } else { "{agent} read it: {what}" },
        "mail.talkReplied" => if zh { "{agent} 回答了：{what}" } else { "{agent} answered: {what}" },
        "mail.handFailed" => if zh { "{agent} 没看成：{what}" } else { "{agent} could not read it: {what}" },
        // Antigravity's turn over with no words from it (agent.rs).
        "mail.agyRefused" => if zh { "它要运行的命令被拒绝了，没有回答" } else { "the command it wanted to run was refused, and it gave no answer" },
        "mail.agySilent" => if zh { "它没有回答" } else { "it gave no answer" },
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
