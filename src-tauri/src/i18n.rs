// The few words the Rust side says itself (the menu, the tray's tooltip),
// from src/shared/i18n.js; the page has the rest.

// 'zh', 'zh-TW' or 'en': as set, or as the system's language (Traditional
// Chinese: zh-TW, zh-Hant…; any other Chinese: zh).
pub fn lang(setting: &str) -> &'static str {
    match setting {
        "zh" => "zh",
        "zh-TW" => "zh-TW",
        "en" => "en",
        _ => of_locale(&sys_locale::get_locale().unwrap_or_default()),
    }
}

fn of_locale(locale: &str) -> &'static str {
    let locale = locale.to_lowercase().replace('_', "-");
    if locale.starts_with("zh-tw") || locale.starts_with("zh-hant") {
        "zh-TW"
    } else if locale == "zh" || locale.starts_with("zh-") || locale.starts_with("zh.") {
        "zh"
    } else {
        "en"
    }
}

// Either Chinese: their punctuation.
pub fn is_chinese(lang: &str) -> bool {
    lang == "zh" || lang == "zh-TW"
}

// The words in lang, of (Simplified, Traditional, English).
pub fn pick<'a>(lang: &str, (zh, tw, en): (&'a str, &'a str, &'a str)) -> &'a str {
    match lang {
        "zh" => zh,
        "zh-TW" => tw,
        _ => en,
    }
}

pub fn t(lang: &str, key: &str) -> String {
    let words = match key {
        "menu.pet" => ("宠物", "寵物", "Pet"),
        "menu.noPets" => ("还没有宠物", "還沒有寵物", "No pets yet"),
        "menu.size" => ("大小", "大小", "Size"),
        "menu.small" => ("小", "小", "Small"),
        "menu.medium" => ("中", "中", "Medium"),
        "menu.large" => ("大", "大", "Large"),
        "menu.walk" => ("空闲时走动", "閒置時走動", "Walk around when idle"),
        "menu.look" => ("眼睛跟着鼠标", "眼睛跟著滑鼠", "Follow the mouse"),
        "menu.backToCorner" => ("回到右下角", "回到右下角", "Back to the corner"),
        "menu.islandDisplay" => ("岛在哪个屏幕", "島在哪個螢幕", "Island on display"),
        "menu.displayAuto" => ("跟着主屏幕", "跟著主螢幕", "The main display"),
        "menu.displayN" => ("屏幕 {n}", "螢幕 {n}", "Display {n}"),
        "menu.displayMain" => ("（主）", "（主）", " (main)"),
        "menu.islandTuck" => ("岛收起来（往下拉才出现）", "島收起來（往下拉才出現）", "Tuck the island away (pull it down)"),
        "phone.low" => ("手机只剩 {n}%，该充电了", "手機只剩 {n}%，該充電了", "The phone is down to {n}%"),
        "menu.ringPhone" => ("让手机响（找手机）", "讓手機響（找手機）", "Ring the phone"),
        "phone.clip" => ("手机传来的文字，已复制", "手機傳來的文字，已複製", "Text from the phone, copied"),
        "phone.got" => ("收到手机的 {name}", "收到手機的 {name}", "From the phone: {name}"),
        "phone.gotWhere" => ("在 下载\\Wakuwaku · 点一下打开", "在 下載\\Wakuwaku · 點一下打開", "In Downloads\\Wakuwaku · click to open"),
        "menu.ask" => ("问她…", "問她…", "Ask her…"),
        "phone.copy" => ("点一下复制", "點一下複製", "click to copy"),
        "menu.dock" => ("右边的竖岛", "右邊的直立島", "Side island"),
        "menu.dnd" => ("勿扰", "勿擾", "Do not disturb"),
        "menu.show" => ("显示宠物", "顯示寵物", "Show pet"),
        "menu.hide" => ("隐藏宠物", "隱藏寵物", "Hide pet"),
        "menu.letOut" => ("放她出来", "放她出來", "Let her out"),
        "menu.callBack" => ("叫她回家", "叫她回家", "Call her back home"),
        "menu.settings" => ("设置…", "設定…", "Settings…"),
        "update.out" => ("有新版本 v{version}", "有新版本 v{version}", "A new version: v{version}"),
        "update.tap" => ("点一下去更新", "點一下去更新", "Tap to update"),
        "crash.out" => ("她上次意外退出了，要发崩溃报告吗？", "她上次意外結束了，要傳送當機報告嗎？", "She quit unexpectedly. Send a crash report?"),
        "crash.tap" => ("点一下看看", "點一下看看", "Tap to see"),
        // A push to the phone (mobile.rs): the agent and how it is, alone.
        "push.waiting" => ("{agent} 在等你", "{agent} 在等你", "{agent} is waiting for you"),
        "push.done" => ("{agent} 做完了", "{agent} 做完了", "{agent} is done"),
        "push.review" => ("{agent} 改好了，来看看", "{agent} 改好了，來看看", "{agent} made changes for you to see"),
        "push.error" => ("{agent} 出错了", "{agent} 出錯了", "{agent} ran into an error"),
        "push.test" => ("Wakuwaku 的推送到了", "Wakuwaku 的推播到了", "Wakuwaku's pushes reach you"),
        "crash.box" => (
            "Wakuwaku 最近连续崩溃了 {n} 次。\n\n要从现在起自动发送崩溃报告吗？报告会公开发到 GitHub 或 Gitee 的 issue 里，用户名、路径、其他程序的名字和窗口标题、邮箱这类内容会先去掉。\n\n以后可以在 设置 → 通用 → 崩溃报告 里关掉。",
            "Wakuwaku 最近連續當機了 {n} 次。\n\n要從現在起自動傳送當機報告嗎？報告會公開發到 GitHub 或 Gitee 的 issue 裡，使用者名稱、路徑、其他程式的名稱和視窗標題、電子郵件這類內容會先去掉。\n\n以後可以在 設定 → 一般 → 當機報告 裡關掉。",
            "Wakuwaku has crashed {n} times in a row.\n\nSend crash reports automatically from now on? They are posted publicly as issues on GitHub or Gitee, with user names, paths, other programs' names and window titles, mail addresses and the like taken out first.\n\nYou can turn this off in Settings → General → Crash reports.",
        ),
        "menu.quit" => ("退出", "結束", "Quit"),
        "menu.newChat" => ("开新对话", "開新對話", "New conversation"),
        "menu.web" => ("开网页版", "開網頁版", "Open on the web"),
        "deny.message" => ("用户在桌宠上拒绝了。", "使用者在桌寵上拒絕了。", "The user denied this on the desktop pet."),
        "codex.waiting" => ("等你在 Wakuwaku 上确认…", "等你在 Wakuwaku 上確認…", "Waiting for your answer on Wakuwaku…"),
        "notify.waiting" => ("{project}需要你确认", "{project}需要你確認", "{project}needs your OK"),
        "notify.done" => ("{project}做完了", "{project}做完了", "{project}is done"),
        "notify.review" => ("{project}改好了，等你看看", "{project}改好了，等你看看", "{project}has changes for you to review"),
        "notify.error" => ("{project}出错了", "{project}出錯了", "{project}hit an error"),
        "notify.project" => ("{project}：", "{project}：", "{project}: "),
        "mood.idle" => ("摸鱼中", "摸魚中", "Taking a break"),
        "mood.working" => ("干活中…", "工作中…", "Working…"),
        "mood.waiting" => ("等你回复！", "等你回覆！", "Waiting for you!"),
        "mood.done" => ("搞定啦 ✓", "搞定囉 ✓", "All done ✓"),
        "mood.review" => ("改好了，来看看 ✓", "改好了，來看看 ✓", "Changes ready for review ✓"),
        "mood.error" => ("呜…出错了", "嗚…出錯了", "Oops, something went wrong"),
        _ => return key.to_string(),
    };
    pick(lang, words).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traditional_chinese_by_its_locale_or_as_set() {
        for l in ["zh-TW", "zh-Hant", "zh-Hant-TW", "zh_TW.UTF-8", "ZH-tw"] {
            assert_eq!(of_locale(l), "zh-TW", "{l}");
        }
        for l in ["zh", "zh-CN", "zh-Hans-CN", "zh_CN.UTF-8"] {
            assert_eq!(of_locale(l), "zh", "{l}");
        }
        for l in ["en-US", "ja-JP", "zhx", ""] {
            assert_eq!(of_locale(l), "en", "{l}");
        }
        assert_eq!((lang("zh-TW"), lang("zh"), lang("en")), ("zh-TW", "zh", "en"));
        assert_eq!(t("zh-TW", "menu.settings"), "設定…");
        assert_eq!(t("zh", "menu.settings"), "设置…");
        assert_eq!(t("fr", "menu.settings"), "Settings…");
        assert_eq!(t("zh-TW", "no.such"), "no.such");
        assert!(is_chinese("zh-TW") && is_chinese("zh") && !is_chinese("en"));
    }
}
