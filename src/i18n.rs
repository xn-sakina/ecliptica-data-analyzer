use serde::{Deserialize, Serialize};

/// Languages supported by the management UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    English,
    Chinese,
}

impl Language {
    pub const ALL: [Self; 2] = [Self::English, Self::Chinese];

    pub fn from_locale(locale: &str) -> Self {
        let language = locale
            .split(['-', '_'])
            .next()
            .unwrap_or(locale)
            .to_ascii_lowercase();
        if language == "zh" {
            Self::Chinese
        } else {
            Self::English
        }
    }

    pub fn system_default() -> Self {
        sys_locale::get_locale()
            .as_deref()
            .map(Self::from_locale)
            .unwrap_or(Self::English)
    }
}

impl Default for Language {
    fn default() -> Self {
        Self::system_default()
    }
}

impl std::fmt::Display for Language {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::English => "English",
            Self::Chinese => "中文",
        })
    }
}

/// One entry in the central bilingual catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextPair {
    pub english: &'static str,
    pub chinese: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub struct VariableCopy {
    pub role: TextPair,
    pub name: &'static str,
    pub description: TextPair,
}

#[derive(Debug, Clone, Copy)]
pub struct VariableCopyGroup {
    pub title: TextPair,
    pub variables: &'static [VariableCopy],
}

#[derive(Debug, Clone, Copy)]
pub struct SyntaxCopy {
    pub title: TextPair,
    pub code: TextPair,
}

impl TextPair {
    pub const fn new(english: &'static str, chinese: &'static str) -> Self {
        Self { english, chinese }
    }

    pub const fn get(self, language: Language) -> &'static str {
        match language {
            Language::English => self.english,
            Language::Chinese => self.chinese,
        }
    }
}

/// The single source of truth for UI copy. Each key must be constructed with
/// both translations, so a language can never be omitted from an entry.
pub mod text {
    use super::TextPair;

    macro_rules! pair {
        ($name:ident, $english:literal, $chinese:literal) => {
            pub const $name: TextPair = TextPair::new($english, $chinese);
        };
    }

    pair!(OVERVIEW, "Overview", "首页");
    pair!(OSC_MESSAGES, "OSC Messages", "OSC 消息");
    pair!(PLAYER_ALERTS, "Boss Alerts", "Boss 提醒");
    pair!(OVERLAY, "Overlay", "悬浮窗");
    pair!(
        OVERLAY_WINDOW_TITLE,
        "Ecliptica Floating Window",
        "Ecliptica 悬浮窗"
    );
    pair!(SYSTEM_LOGS, "System Logs", "系统日志");
    pair!(WORKSPACE, "WORKSPACE", "工作区");
    pair!(
        SAVE_SUCCESS,
        "Settings saved; OSC will update automatically",
        "设置已保存，OSC 会自动更新"
    );
    pair!(SAVE_FAILED, "Failed to save", "保存失败");
    pair!(LANGUAGE_SAVED, "Language saved", "语言已保存");
    pair!(LANGUAGE_TOOLTIP, "Language / 语言", "Language / 语言");
    pair!(
        LANGUAGE_SAVED_LOG,
        "Language preference saved",
        "语言偏好已保存"
    );
    pair!(
        SAVE_FAILED_VIEW_ERROR,
        "Save failed; view the full error",
        "保存失败，请查看完整错误"
    );
    pair!(
        UNSAVED_CHANGES,
        "You have unsaved changes; save to apply them",
        "有未保存的修改，请保存后应用"
    );
    pair!(RESTORE_DEFAULTS, "Restore defaults", "恢复默认设置");
    pair!(
        RESTORE_DEFAULTS_HINT,
        "Load defaults; save to apply them",
        "载入默认值；点击保存后生效"
    );
    pair!(SAVE_AND_APPLY, "Save changes", "保存修改");
    pair!(SAVED, "Saved", "已保存");
    pair!(
        SAVE_AND_APPLY_HINT,
        "Save all changes and apply now",
        "保存全部修改并立即应用"
    );
    pair!(
        NO_CHANGES_TO_SAVE,
        "There are no changes to save",
        "没有需要保存的修改"
    );
    pair!(VIEW_FULL_ERROR, "View full error", "查看完整错误");
    pair!(ECLIPTICA_DETECTED, "Inside Ecliptica", "已进入 Ecliptica");
    pair!(
        WAITING_FOR_ECLIPTICA,
        "Waiting to enter Ecliptica",
        "等待进入 Ecliptica"
    );
    pair!(
        RESTORE_DEFAULTS_TITLE,
        "Restore default settings?",
        "恢复默认设置？"
    );
    pair!(
        RESTORE_DEFAULTS_DESCRIPTION,
        "This clears your unsaved changes. Save afterward to keep the restored defaults.",
        "当前未保存的修改会被清除。恢复后记得保存。"
    );
    pair!(CANCEL, "Cancel", "取消");
    pair!(CLOSE_DIALOG, "Close dialog", "关闭弹窗");
    pair!(RESTORE, "Restore", "确认恢复");
    pair!(
        DEFAULTS_RESTORED,
        "Defaults restored; save to apply",
        "已恢复默认值，保存后生效"
    );
    pair!(TEMPLATE_SYNTAX_HELP, "Template syntax help", "模板语法帮助");
    pair!(DEVELOPER_LOGS, "Developer Logs", "开发者日志");
    pair!(
        DEVELOPER_LOGS_DESCRIPTION,
        "For diagnosing changes in external log formats; these do not necessarily indicate an app error.",
        "仅用于排查外部日志格式变化，不代表应用运行异常。"
    );
    pair!(
        NO_DEVELOPER_DIAGNOSTICS,
        "No developer diagnostics",
        "暂无开发者诊断"
    );
    pair!(UNPIN_WINDOW, "Unpin window", "取消固定窗口");
    pair!(PIN_WINDOW, "Pin window", "固定窗口");
    pair!(
        UNPIN_WINDOW_HINT,
        "Return to a normal window",
        "恢复为普通窗口"
    );
    pair!(
        PIN_WINDOW_HINT,
        "Keep the main window above other windows",
        "让主窗口始终显示在其他窗口上方"
    );
    pair!(LIVE_DPS, "Current DPS", "当前 DPS");
    pair!(AVERAGE_DPS_30S, "Average DPS", "平均 DPS");
    pair!(ROUND_EFFECTIVE_DPS, "Active DPS", "持续输出 DPS");
    pair!(ROUND_BURST_10S, "Best average DPS", "最佳平均 DPS");
    pair!(ROUND_DAMAGE_TAKEN, "Damage taken this round", "本回合承伤");
    pair!(BOSS_LOCK, "BOSS LOCK", "Boss 锁定");
    pair!(SESSION_DPS_CHART, "DPS trend", "DPS 走势");
    pair!(
        PREVIOUS_ROUND_REPORT,
        "Previous round report",
        "上一回合战报"
    );
    pair!(DURATION, "Duration", "用时");
    pair!(TOTAL_DAMAGE, "Total damage", "总伤害");
    pair!(EFFECTIVE_DPS, "Active DPS", "持续输出 DPS");
    pair!(BURST_10S, "Best average DPS", "最佳平均 DPS");
    pair!(
        EFFECTIVE_DPS_GROWTH,
        "Active DPS change",
        "持续输出 DPS 变化"
    );
    pair!(DAMAGE_TAKEN, "Damage taken", "承伤");
    pair!(LONGEST_STANDSTILL, "Longest standstill", "最长站桩");
    pair!(GAME_LOG, "Game log", "游戏日志");
    pair!(SOUND_SOFT, "Soft", "柔和");
    pair!(SOUND_CRISP, "Crisp", "清脆");
    pair!(SOUND_PROMINENT, "Prominent", "强提醒");
    pair!(PHASE_OUTSIDE, "Outside Ecliptica", "未进入 Ecliptica");
    pair!(
        PHASE_SYNCING,
        "Reading current progress",
        "正在读取当前进度"
    );
    pair!(PHASE_LOBBY, "Lobby / Upgrading", "大厅 / 升级中");
    pair!(PHASE_COMBAT, "In combat", "战斗中");
    pair!(SEND_SETTINGS, "Send settings", "发送设置");
    pair!(
        SEND_SETTINGS_DESCRIPTION,
        "VRChat listens at 127.0.0.1:9000 by default",
        "VRChat 默认接收地址为 127.0.0.1:9000"
    );
    pair!(ENABLE_OSC, "Enable OSC", "启用 OSC");
    pair!(AWAY_MODE, "Away mode", "临时外出");
    pair!(OPEN_AWAY_MODE, "Go away", "临时外出");
    pair!(AWAY_REASON, "Reason", "外出原因");
    pair!(AWAY_REASON_TAKEOUT, "Pick up delivery", "拿外卖");
    pair!(AWAY_REASON_RESTROOM, "Restroom", "去卫生间");
    pair!(AWAY_REASON_CUSTOM, "Custom", "自定义");
    pair!(AWAY_DURATION, "Return time", "预计时间");
    pair!(AWAY_ONE_MINUTE, "1 minute", "1 分钟");
    pair!(AWAY_THREE_MINUTES, "3 minutes", "3 分钟");
    pair!(AWAY_FIVE_MINUTES, "5 minutes", "5 分钟");
    pair!(AWAY_TEN_MINUTES, "10 minutes", "10 分钟");
    pair!(AWAY_CUSTOM_MESSAGE, "Custom message", "自定义消息");
    pair!(AWAY_MESSAGE, "Message preview", "消息预览");
    pair!(
        AWAY_TIME_VARIABLE_HINT,
        "Use {{time}} for the countdown.",
        "使用 {{time}} 显示倒计时。"
    );
    pair!(
        AWAY_MESSAGE_REQUIRED,
        "Enter an away message.",
        "请输入外出消息。"
    );
    pair!(
        AWAY_OSC_REQUIRED,
        "Enable and save OSC first.",
        "请先启用并保存 OSC。"
    );
    pair!(ENTER_AWAY_MODE, "Start away mode", "开始外出");
    pair!(EXIT_AWAY_MODE, "End away mode", "结束外出");
    pair!(AWAY_MODE_ACTIVE, "Away mode is active", "正在外出");
    pair!(EXIT_AWAY_MODE_TITLE, "End away mode?", "结束外出？");
    pair!(
        EXIT_AWAY_MODE_DESCRIPTION,
        "Your away message will stop sending.",
        "外出消息将停止发送。"
    );
    pair!(KEEP_AWAY_MODE, "Stay", "继续外出");
    pair!(CONFIRM_EXIT_AWAY_MODE, "End and close", "结束外出");
    pair!(
        AWAY_TAKEOUT_MESSAGE,
        "Sorry, picking up a delivery\nBack in: {{time}}",
        "抱歉，拿个外卖\n距回来：{{time}}"
    );
    pair!(
        AWAY_RESTROOM_MESSAGE,
        "Sorry, going to the restroom\nBack in: {{time}}",
        "抱歉，去趟卫生间\n距回来：{{time}}"
    );
    pair!(
        AWAY_CUSTOM_DEFAULT_MESSAGE,
        "Sorry, something came up. I'll be back soon\nBack in: {{time}}",
        "抱歉，临时有事，很快回来\n距回来：{{time}}"
    );
    pair!(ENABLE_HEART_RATE, "Receive heart rate", "接收心率");
    pair!(
        ENABLE_HEART_RATE_HINT,
        "Receive data from vrchat-fast-heart. Turn on “Report to other apps” there, then save these settings.",
        "从 vrchat-fast-heart 接收数据。请先在该应用中开启“向其他应用报告（Report to other apps）”，然后保存设置。"
    );
    pair!(HEART_RATE_AUXILIARY, "Heart rate", "心率");
    pair!(
        HEART_RATE_SETUP_GUIDE,
        "View heart-rate setup guide",
        "查看心率设置指南"
    );
    pair!(SEND_INTERVAL, "Send interval", "发送频率");
    pair!(TARGET_ADDRESS, "Destination", "目标地址");
    pair!(EVERY_SECONDS, "Every {seconds} seconds", "每 {seconds} 秒");
    pair!(PLAYER_IDENTITY, "Your VRChat name", "你的 VRChat 名字");
    pair!(
        PLAYER_IDENTITY_DESCRIPTION,
        "Boss alerts need your name to identify you",
        "不填名字，就不会提醒你被 Boss 锁定"
    );
    pair!(
        DISPLAY_NAME_PLACEHOLDER,
        "Enter the name you use in VRChat",
        "输入你在 VRChat 里用的名字"
    );
    pair!(ALERT_SOUNDS, "Alert sounds", "提示音");
    pair!(VOLUME, "Volume", "音量");
    pair!(ALERT_VOLUME, "Alert volume", "提示音音量");
    pair!(LOCK_SOUND, "When the Boss targets you", "Boss 锁定你时");
    pair!(
        RELEASE_SOUND,
        "When the Boss changes target",
        "Boss 转移目标时"
    );
    pair!(PREVIEW_SOUND, "Preview", "试听");
    pair!(WINDOW_BEHAVIOR, "Floating window settings", "悬浮窗配置");
    pair!(DRAGGABLE, "Allow dragging", "允许拖动");
    pair!(
        DRAG_OVERLAY_HINT,
        "Drag anywhere on the floating window to move it.",
        "拖住悬浮窗的任意位置即可移动。"
    );
    pair!(OVERLAY_SIZE, "Floating window size", "悬浮窗大小");
    pair!(SCREEN_POSITION, "Adjust position manually", "手动调整位置");
    pair!(HORIZONTAL_POSITION, "Left / right", "左右位置");
    pair!(VERTICAL_POSITION, "Up / down", "上下位置");
    pair!(PIXELS_SUFFIX, " px", " 像素");
    pair!(
        DEVELOPER_LOGS_HINT,
        "Open protocol compatibility and other specialized diagnostics",
        "打开协议兼容性等专用诊断"
    );
    pair!(EVENT_STREAM, "System log", "系统日志");
    pair!(NO_SYSTEM_EVENTS, "Nothing to show yet", "暂时没有日志");
    pair!(INFO, "Info", "信息");
    pair!(WARNING, "Warning", "警告");
    pair!(ERROR, "Error", "错误");
    pair!(NORMAL_MESSAGE_TEMPLATE, "Combat message", "战斗消息");
    pair!(TEMPLATE_PRESET, "Choose template", "选择模板");
    pair!(PRESET_NAME, "Template name", "模板名称");
    pair!(PRESET_FALLBACK, "Preset {index}", "预设 {index}");
    pair!(MESSAGE_PRESET_OUTPUT, "DPS", "输出职");
    pair!(MESSAGE_PRESET_TANK, "Tank", "承伤职");
    pair!(MESSAGE_PRESET_BACKUP, "Backup", "备用");
    pair!(REPORT_PRESET_OUTPUT, "DPS Report", "输出战报");
    pair!(REPORT_PRESET_TANK, "Tank Report", "承伤战报");
    pair!(REPORT_PRESET_BACKUP, "Backup Report", "备用战报");
    pair!(
        RESET_SELECTED_PRESET,
        "Restore default content",
        "恢复默认内容"
    );
    pair!(
        RESET_MESSAGE_PRESET_HINT,
        "Restore only the selected combat-message template",
        "只恢复当前战斗消息的默认内容"
    );
    pair!(
        RESET_REPORT_PRESET_HINT,
        "Restore only the selected round report",
        "只恢复当前战报的默认内容"
    );
    pair!(
        RESET_MESSAGE_PRESET_TITLE,
        "Restore this combat message?",
        "恢复这条战斗消息？"
    );
    pair!(
        RESET_REPORT_PRESET_TITLE,
        "Restore this round report?",
        "恢复这份战报？"
    );
    pair!(
        RESET_MESSAGE_PRESET_DESCRIPTION,
        "Only “{name}” will be restored. Other templates stay unchanged. Save afterward to keep the change.",
        "只恢复「{name}」的默认内容，其他模板不变。恢复后记得保存。"
    );
    pair!(
        RESET_REPORT_PRESET_DESCRIPTION,
        "Only “{name}” will be restored. Other templates stay unchanged. Save afterward to keep the change.",
        "只恢复「{name}」的默认内容，其他模板不变。恢复后记得保存。"
    );
    pair!(
        MESSAGE_PRESET_RESET,
        "Combat message “{name}” restored; save to apply",
        "战斗消息「{name}」已恢复默认，保存后生效"
    );
    pair!(
        REPORT_PRESET_RESET,
        "Round report “{name}” restored; save to apply",
        "战报「{name}」已恢复默认，保存后生效"
    );
    pair!(
        PRESET_SWITCHED,
        "Switched to “{name}”; save to apply",
        "已切换到「{name}」，保存后应用"
    );
    pair!(
        PRESET_NAME_HINT,
        "Up to {max} characters",
        "最多 {max} 个字符"
    );
    pair!(
        LIVE_VARIABLES_HINT,
        "Click a variable to copy it. “Show when” controls when text appears.",
        "点一下变量就会复制。“显示条件”可以让内容只在需要时出现。"
    );
    pair!(ROUND_REPORT_TEMPLATE, "Round report", "回合战报");
    pair!(REPORT_PRESET, "Choose report", "选择战报");
    pair!(
        REPORT_VARIABLES_HINT,
        "Click a variable to copy it. These are available after a round ends.",
        "点一下变量就会复制。这些内容会在回合结束后显示。"
    );
    pair!(LIVE_PREVIEW, "Live preview", "实时预览");
    pair!(SIMULATED_STATE, "Preview", "预览内容");
    pair!(PREVIEW_NORMAL, "Combat message", "战斗消息");
    pair!(PREVIEW_ROUND_REPORT, "Round report", "回合战报");
    pair!(
        EMPTY_MESSAGE,
        "The current message is empty and will not be sent",
        "当前消息为空，不会发送"
    );
    pair!(TEMPLATE_ERROR, "Template error", "模板错误");
    pair!(
        CHART_WAITING_FIRST_SECOND,
        "Waiting for data…",
        "正在等待数据…"
    );
    pair!(
        CHART_ENTER_ECLIPTICA,
        "Enter Ecliptica to begin recording",
        "进入 Ecliptica 后开始记录"
    );
    pair!(
        CHART_WAITING_ROUND,
        "Waiting for combat to begin…",
        "等待战斗开始…"
    );
    pair!(
        CHART_WAITING_DATA,
        "Waiting for current round data…",
        "正在等待当前回合数据…"
    );
    pair!(
        CHART_CURRENT_ESTIMATED_ROUND,
        "Estimated round {step} in progress",
        "预计当前为第 {step} 回合"
    );
    pair!(
        CHART_FINISHED_ESTIMATED_ROUND,
        "Estimated round {step} just ended",
        "预计第 {step} 回合刚结束"
    );
    pair!(CHART_CURRENT_ROUND, "Current round", "当前回合");
    pair!(CHART_FINISHED_ROUND, "Round just ended", "回合刚结束");
    pair!(DPS_AVERAGE, "Average DPS", "平均 DPS");
    pair!(DPS_PER_SECOND, "Current DPS", "当时 DPS");
    pair!(DPS_ROUND_PEAK, "Highest DPS", "最高 DPS");
    pair!(
        DPS_CHART_TOOLTIP,
        "{time}\nCurrent DPS: {raw}\nAverage DPS: {trend}",
        "{time}\n当时 DPS：{raw}\n平均 DPS：{trend}"
    );
    pair!(
        DPS_CHART_PEAK_TOOLTIP,
        "{time}\nHighest DPS: {raw}\nAverage DPS: {trend}",
        "{time}\n最高 DPS：{raw}\n平均 DPS：{trend}"
    );
    pair!(
        DPS_CHART_ACCESSIBILITY,
        "{round}. Bars show current DPS and the line shows average DPS. Highest DPS: {peak}, at {time}.",
        "{round}。柱形是当时 DPS，曲线是平均 DPS。最高 DPS 为 {peak}，出现在{time}。"
    );
    pair!(SESSION_TIME, "Session time", "本局时间");
    pair!(
        ELAPSED_HOURS_MINUTES,
        "{hours}h {minutes}m",
        "{hours}小时 {minutes}分"
    );
    pair!(ELAPSED_HOURS, "{hours}h", "{hours}小时");
    pair!(
        ELAPSED_MINUTES_SECONDS,
        "{minutes}m {seconds}s",
        "{minutes}分 {seconds}秒"
    );
    pair!(ELAPSED_MINUTES, "{minutes}m", "{minutes}分");
    pair!(ELAPSED_SECONDS, "{seconds}s", "{seconds}秒");
    pair!(SECONDS_VALUE, "{seconds}s", "{seconds}秒");
    pair!(ROUND_REPORT_HEADING, "Round report", "回合战报");
    pair!(
        RETURNED_TO_LOBBY,
        "Returned to the upgrade lobby",
        "已返回升级大厅"
    );
    pair!(TIME_USED, "Time", "用时");
    pair!(LATEST, "Current", "当前");
    pair!(EFFECTIVE, "Active", "持续输出");
    pair!(BURST_10S_SHORT, "Best", "最佳");
    pair!(DAMAGE_TAKEN_SHORT, "Taken", "承伤");
    pair!(
        ROUND_DAMAGE_TAKEN_TOTAL,
        "Total damage taken this round",
        "本回合承伤"
    );
    pair!(EXACT_VALUE, "Exact value", "精确值");
    pair!(LOG_FILE, "File location", "文件位置");
    pair!(OPEN_FOLDER, "Open folder", "打开文件夹");
    pair!(SEARCHING, "Searching…", "正在查找…");
    pair!(LOG_NOT_FOUND, "No game log found yet", "还没找到游戏日志");
    pair!(
        LOG_MISSING,
        "The game log or its folder no longer exists",
        "游戏日志或所在文件夹已不存在"
    );
    pair!(
        OPEN_LOG_FOLDER_FINDER,
        "Open the game-log folder in Finder",
        "在 Finder 中打开游戏日志所在文件夹"
    );
    pair!(
        OPEN_LOG_FOLDER_EXPLORER,
        "Open the game-log folder in File Explorer",
        "在文件资源管理器中打开游戏日志所在文件夹"
    );
    pair!(
        OPEN_LOG_FOLDER_MANAGER,
        "Open the game-log folder in the file manager",
        "在文件管理器中打开游戏日志所在文件夹"
    );
    pair!(
        SAVE_ERROR_GUIDANCE,
        "Follow the message below, then save again.",
        "按下面的提示修改后，再保存一次。"
    );
    pair!(
        COPY_VARIABLE_HINT,
        "Click to copy {token}",
        "点击复制 {token}"
    );
    pair!(
        CLIPBOARD_UNAVAILABLE,
        "System clipboard is unavailable",
        "系统剪贴板不可用"
    );
    pair!(VARIABLE_COPIED, "Variable copied", "变量已复制");
    pair!(COPY_FAILED, "Copy failed; try again", "复制失败，请重试");
    pair!(ROLE_CONDITION, "Show when", "显示条件");
    pair!(ROLE_VALUE, "Value", "数值");
    pair!(ROLE_TEXT, "Text", "文本");
    pair!(ROLE_STATUS, "Status", "状态");
    pair!(ROLE_JUDGMENT, "Check", "判断");
    pair!(ROLE_DISPLAY, "Display", "显示");
    pair!(
        OPENED_LOG_FOLDER,
        "Opened game-log folder",
        "已打开游戏日志所在文件夹"
    );
    pair!(
        OPEN_LOG_FOLDER_FAILED,
        "Failed to open the game-log folder",
        "打开游戏日志所在文件夹失败"
    );
    pair!(
        DEVELOPER_MODE_ENABLED,
        "Developer mode enabled",
        "开发者模式已开启"
    );
    pair!(
        DEVELOPER_MODE_DISABLED,
        "Developer mode disabled",
        "开发者模式已关闭"
    );
    pair!(
        CONFIG_SAVED_LOG,
        "Settings saved; OSC will update automatically",
        "设置已保存，OSC 会自动更新"
    );
    pair!(ROUND_TICK, "{step} rounds", "{step} 回合");
    pair!(COMPACT_TEN_THOUSAND, "K", "万");
    pair!(COMPACT_HUNDRED_MILLION, "M", "亿");
    pair!(STATUS_SEARCHING, "SEARCHING", "查找中");
    pair!(STATUS_RECOVERING, "RECOVERING", "恢复中");
    pair!(STATUS_LIVE, "LIVE", "正常");
    pair!(STATUS_STALE, "STALE", "等待数据");
    pair!(STATUS_ERROR, "ERROR", "错误");
    pair!(
        AUDIO_INIT_FAILED,
        "Alert sounds are unavailable",
        "暂时无法播放提示音"
    );
    pair!(
        AUDIO_PLAYBACK_FAILED,
        "Failed to play alert sound",
        "提示音播放失败"
    );
    pair!(
        WASD_INIT_FAILED,
        "Movement data is unavailable",
        "暂时无法记录移动"
    );
    pair!(
        WASD_KEYBOARD_INIT_FAILED,
        "Movement data is unavailable",
        "暂时无法记录移动"
    );
    pair!(
        WASD_INTERRUPTED,
        "Movement tracking stopped unexpectedly",
        "移动记录意外停止"
    );
    pair!(
        LOG_DISCOVERY_FAILED,
        "Failed to find VRChat logs",
        "查找 VRChat 日志失败"
    );
    pair!(
        LOG_REPLACED,
        "The game log changed and has been reloaded",
        "游戏日志已更新，数据已重新读取"
    );
    pair!(
        LOG_ID_FAILED,
        "Couldn't check whether the game log changed",
        "无法确认游戏日志是否更新"
    );
    pair!(
        LOG_TRUNCATED,
        "The game log was reset and has been reloaded",
        "游戏日志已重置，数据已重新读取"
    );
    pair!(
        LOG_READ_FAILED,
        "Failed to read log; searching again",
        "读取日志失败，准备重新查找"
    );
    pair!(
        LOG_FOUND,
        "Game log found; restoring current data",
        "已找到游戏日志，正在恢复当前数据"
    );
    pair!(
        LOG_PROTOCOL_DEGRADED,
        "Log protocol compatibility degraded",
        "日志协议兼容性降级"
    );
    pair!(
        LOG_PROTOCOL_DEGRADED_SUFFIX,
        "The app will continue running; affected fields show 0/unknown",
        "软件将继续运行，相关字段显示 0/未知"
    );
    pair!(
        LOG_LIVE,
        "Game data is ready; OSC can send messages",
        "游戏数据已准备好，OSC 可以发送消息"
    );
    pair!(
        LOG_STALE,
        "Waiting for new game data; OSC is paused",
        "正在等待新的游戏数据，OSC 已暂停"
    );
    pair!(
        LOG_SEARCHING,
        "No VRChat log found; retrying",
        "未找到 VRChat 日志，将继续重试"
    );
    pair!(
        LOG_RECOVERING,
        "Restoring current game data; alerts are paused",
        "正在恢复当前游戏数据，提醒已暂停"
    );
    pair!(
        LOG_ERROR,
        "Log reading failed; OSC is paused",
        "日志读取发生错误，OSC 已暂停"
    );
    pair!(OSC_INIT_FAILED, "OSC couldn't start", "OSC 启动失败");
    pair!(
        OSC_STATE_PACKET_SUBMITTED,
        "OSC state-change packet submitted; older pending messages were discarded",
        "OSC 状态切换包已提交，旧待发送消息已丢弃"
    );
    pair!(OSC_SEND_FAILED, "OSC send failed", "OSC 发送失败");
    pair!(
        HEART_RATE_SERVER_FAILED,
        "Heart rate couldn't start. Restart the app and try again.",
        "心率功能未能启动，请重启应用后重试。"
    );
    pair!(
        HEART_RATE_NO_PORT,
        "Heart rate is temporarily unavailable. The app will keep trying.",
        "心率暂时不可用，应用会继续尝试恢复。"
    );
    pair!(
        HEART_RATE_CONNECTED,
        "Heart rate received",
        "已收到心率数据"
    );
    pair!(
        HEART_RATE_DISCONNECTED,
        "Heart rate signal lost. Check your heart-rate app.",
        "心率连接已中断，请检查心率应用。"
    );
    pair!(
        HEART_RATE_WAITING,
        "No heart rate received. Check that your heart-rate app is open and sharing data.",
        "尚未收到心率，请确认心率应用已打开并允许共享数据。"
    );
    pair!(
        HEART_RATE_VARIABLE_OFFLINE,
        "Heart rate isn't available yet. Enable it, save the settings, and make sure your heart-rate app is sharing data.",
        "心率暂不可用。请开启心率并保存设置，同时确认心率应用正在共享数据。"
    );
    pair!(
        SINGLE_INSTANCE_FAILED,
        "The app couldn't start correctly",
        "应用未能正常启动"
    );
    pair!(
        ALREADY_RUNNING,
        "Ecliptica Data Analyzer is already running",
        "Ecliptica Data Analyzer 已经在运行"
    );
    pair!(
        EXIT_HANDLER_FAILED,
        "The app couldn't prepare to close safely",
        "应用未能正常准备退出"
    );
    pair!(
        VOLUME_INVALID,
        "Alert volume must be between 0 and 1",
        "提示音量必须在 0 到 1 之间"
    );
    pair!(
        OVERLAY_SCALE_INVALID,
        "Floating window size must be between 0.5× and 3×",
        "悬浮窗大小必须在 0.5 到 3 倍之间"
    );
    pair!(
        STALE_TIME_INVALID,
        "The wait-for-data time must be between 2 and 300 seconds",
        "等待数据的时间必须在 2 到 300 秒之间"
    );
    pair!(
        OSC_ADDRESS_INVALID,
        "Invalid OSC address; expected something like 127.0.0.1:9000",
        "OSC 地址格式无效，应类似 127.0.0.1:9000"
    );
    pair!(
        REPORT_TEMPLATE_INVALID,
        "Round report template is invalid",
        "回合战报模板无效"
    );
    pair!(MESSAGE_TEMPLATE_KIND, "Message template", "消息模板");
    pair!(
        REPORT_TEMPLATE_KIND,
        "Round report template",
        "回合战报模板"
    );
    pair!(
        TEMPLATE_SYNTAX_ERROR,
        "Message template syntax error",
        "消息模板语法错误"
    );
    pair!(
        TEMPLATE_UNKNOWN_VARIABLE,
        "Message template contains an unknown variable",
        "消息模板包含未知变量"
    );
    pair!(
        CONFIG_DIR_UNAVAILABLE,
        "Couldn't find a location to save settings",
        "找不到保存设置的位置"
    );
    pair!(
        CONFIG_READ_FAILED,
        "Failed to read settings: {path}",
        "读取设置失败：{path}"
    );
    pair!(
        CONFIG_JSON_CORRUPT,
        "The settings file is corrupted",
        "设置文件已损坏"
    );
    pair!(
        CONFIG_BACKUP_FAILED,
        "The damaged settings couldn't be backed up to {path}",
        "设置文件已损坏，也无法备份到 {path}"
    );
    pair!(
        CONFIG_RECOVERED,
        "The settings were damaged, so defaults were restored. The original file is at {path}",
        "设置文件已损坏，已恢复默认设置。原文件保存在 {path}"
    );
    pair!(
        CONFIG_DIR_CREATE_FAILED,
        "Failed to create the settings folder: {path}",
        "创建设置文件夹失败：{path}"
    );
    pair!(
        CONFIG_TEMP_CREATE_FAILED,
        "Failed to prepare the settings file",
        "准备设置文件失败"
    );
    pair!(
        CONFIG_SERIALIZE_FAILED,
        "Failed to prepare settings for saving",
        "保存设置前的准备失败"
    );
    pair!(
        CONFIG_TEMP_SYNC_FAILED,
        "Failed to finish saving settings",
        "设置未能保存完成"
    );
    pair!(
        CONFIG_REPLACE_FAILED,
        "Failed to replace the settings file: {path}",
        "替换设置文件失败：{path}"
    );
    pair!(
        USERPROFILE_MISSING,
        "Couldn't find your user folder",
        "找不到你的用户文件夹"
    );
    pair!(
        CONFIG_VERSION_UNSUPPORTED,
        "These settings were created by a newer app version ({version}); this version supports up to {supported}",
        "这些设置来自更新的应用版本（{version}），当前版本最高支持 {supported}"
    );
    pair!(
        MESSAGE_PRESET_RANGE_INVALID,
        "Message template preset must be between 1 and {count}",
        "消息模板预设必须在 1 到 {count} 之间"
    );
    pair!(
        REPORT_PRESET_RANGE_INVALID,
        "Round report template preset must be between 1 and {count}",
        "回合战报模板预设必须在 1 到 {count} 之间"
    );
    pair!(
        MESSAGE_PRESET_INVALID,
        "Message template preset {index} is invalid",
        "消息模板预设 {index} 无效"
    );
    pair!(
        REPORT_PRESET_INVALID,
        "Round report template preset {index} is invalid",
        "回合战报模板预设 {index} 无效"
    );
    pair!(
        PRESET_NAME_EMPTY,
        "{kind} preset {index} name cannot be empty",
        "{kind}预设 {index} 的名称不能为空"
    );
    pair!(
        PRESET_NAME_TOO_LONG,
        "{kind} preset {index} name cannot exceed {max} characters",
        "{kind}预设 {index} 的名称不能超过 {max} 个字符"
    );
    pair!(
        LOG_ACCESS_FAILED,
        "Cannot access log file",
        "无法访问日志文件"
    );
    pair!(
        LOG_PATH_NOT_FILE,
        "Log path is not a file",
        "日志路径不是文件"
    );
    pair!(
        CURRENT_DIR_FAILED,
        "Cannot determine the current directory",
        "无法获取当前目录"
    );
    pair!(
        LOG_PARENT_MISSING,
        "The game log has no folder",
        "找不到游戏日志所在文件夹"
    );
    pair!(
        LOG_FOLDER_MISSING,
        "Game-log folder does not exist",
        "游戏日志所在文件夹不存在"
    );
    pair!(
        FILE_MANAGER_LAUNCH_FAILED,
        "Cannot launch the system file manager to open",
        "无法启动系统文件管理器打开"
    );
    pair!(
        EMPTY_COMBAT_REPLACEMENT,
        "【In combat】Waiting for data…",
        "【战斗中】等待数据…"
    );
    pair!(EMPTY_REPORT_REPLACEMENT, "【Round ended】", "【回合结束】");
    pair!(
        DIAGNOSTIC_STAGE_DETAILS,
        "Stage logs are still recognized, but phase/class details changed; round state is preserved and stage estimation is unavailable",
        "阶段日志仍可识别，但 phase/class 详情格式已变化；回合状态保留，阶段估算降级为未知"
    );
    pair!(
        DIAGNOSTIC_BOSS,
        "Boss log format changed; Boss and target information are unavailable",
        "Boss 日志格式已变化，Boss/锁定信息已降级为空"
    );
    pair!(
        DIAGNOSTIC_BOSS_DEFEATED,
        "Boss-defeat log format changed; target state will wait for a later event to clear",
        "Boss 击败日志格式已变化，锁定状态会等待后续事件清理"
    );
    pair!(
        DIAGNOSTIC_OWNERSHIP,
        "Ownership log format changed; Boss Lock is unavailable",
        "所有权日志格式已变化，Boss Lock 已降级为空"
    );
    pair!(
        DIAGNOSTIC_VALUE,
        "The {code} log format or numeric value changed; this entry was treated as missing",
        "{code} 日志格式或数值已变化，本条数据按缺失处理"
    );
    pair!(
        DIAGNOSTIC_TIMESTAMP,
        "A {code} log appeared, but its timestamp format was not recognized; the entry was safely skipped",
        "{code} 日志已出现，但时间戳格式无法识别；本条数据已安全跳过"
    );
    pair!(
        DIAGNOSTIC_INTERMISSION_MISSING,
        "The next stage appeared during combat without an intermission/lobby log; the incomplete previous round was discarded and a new round started safely",
        "战斗中直接观察到下一阶段，未观察到 intermission/lobby 日志；上一回合按不完整数据丢弃并安全开始新回合"
    );
    pair!(
        DIAGNOSTIC_COMBAT_METRICS_MISSING,
        "No damage dealt or taken logs appeared during the complete stage; values use 0/unknown",
        "完整阶段内未观察到输出或承伤日志；数值按 0/未知处理（可能确实无伤害，也可能是游戏停止打印相关日志）"
    );
    pair!(
        DIAGNOSTIC_ROOM_PHASE_MISSING,
        "No stage/intermission/boss/damage phase signal appeared after entering Ecliptica; safely using the lobby state with 0/unknown values",
        "进入 Ecliptica 后未观察到 stage/intermission/boss/damage 阶段信号；已安全降级为大厅，数值保持 0/未知"
    );
    pair!(
        BACKGROUND_THREAD_FAILED,
        "A background feature stopped unexpectedly",
        "后台功能意外停止"
    );
}

const fn p(english: &'static str, chinese: &'static str) -> TextPair {
    TextPair::new(english, chinese)
}

macro_rules! variable {
    ($role_en:literal, $role_zh:literal, $name:literal, $en:literal, $zh:literal) => {
        VariableCopy {
            role: p($role_en, $role_zh),
            name: $name,
            description: p($en, $zh),
        }
    };
}

pub const HEART_RATE_VARIABLE_GROUPS: &[VariableCopyGroup] = &[VariableCopyGroup {
    title: p("Heart rate", "心率"),
    variables: &[
        variable!(
            "Value",
            "数值",
            "heart_rate",
            "Current heart rate. Displays “-” when no data is available.",
            "当前心率；没有可用数据时显示“-”。"
        ),
        variable!(
            "Show when",
            "显示条件",
            "has_heart_rate",
            "Show this content when heart-rate data is available.",
            "有心率数据时显示这段内容。"
        ),
    ],
}];

pub const LIVE_VARIABLE_GROUPS: &[VariableCopyGroup] = &[
    VariableCopyGroup {
        title: p("Current DPS", "当前 DPS"),
        variables: &[
            variable!(
                "Value",
                "数值",
                "latest_dps",
                "Your DPS right now. It returns to 0 when you stop attacking.",
                "你现在的 DPS；停止输出后会回到 0。"
            ),
            variable!(
                "Show when",
                "显示条件",
                "has_latest_dps",
                "Show this content when current DPS is available.",
                "有当前 DPS 时显示这段内容。"
            ),
        ],
    },
    VariableCopyGroup {
        title: p("Average DPS", "平均 DPS"),
        variables: &[
            variable!(
                "Value",
                "数值",
                "avg_dps",
                "Your recent average DPS. Displays “-” when unavailable.",
                "你最近一段时间的平均 DPS；还没有数据时显示“-”。"
            ),
            variable!(
                "Show when",
                "显示条件",
                "has_avg_dps",
                "Show this content when average DPS is available.",
                "有平均 DPS 时显示这段内容。"
            ),
        ],
    },
    VariableCopyGroup {
        title: p("Current round average DPS", "本回合平均 DPS"),
        variables: &[
            variable!(
                "Value",
                "数值",
                "round_avg_dps",
                "Average DPS from the first damage of this round until now. Displays “-” when unavailable.",
                "本回合从开始输出到现在的平均 DPS。没有数据时显示“-”。"
            ),
            variable!(
                "Show when",
                "显示条件",
                "has_round_avg_dps",
                "Show this content when this round's average DPS is available.",
                "有本回合平均 DPS 时显示这段内容。"
            ),
        ],
    },
    VariableCopyGroup {
        title: p("Active DPS", "持续输出 DPS"),
        variables: &[
            variable!(
                "Value",
                "数值",
                "round_effective_dps",
                "Your average DPS while attacking; walking and waiting do not lower it.",
                "只看你攻击时的平均 DPS，走路和等待不算。"
            ),
            variable!(
                "Show when",
                "显示条件",
                "has_round_effective_dps",
                "Show this content when active DPS is available.",
                "有持续输出 DPS 时显示这段内容。"
            ),
        ],
    },
    VariableCopyGroup {
        title: p("Best average DPS", "最佳平均 DPS"),
        variables: &[
            variable!(
                "Value",
                "数值",
                "round_burst_10s",
                "Your best average DPS this round. Displays “-” until enough data is available.",
                "本回合表现最好的一段平均 DPS；数据不足时显示“-”。"
            ),
            variable!(
                "Show when",
                "显示条件",
                "has_round_burst_10s",
                "Show this content when best average DPS is available.",
                "有最佳平均 DPS 时显示这段内容。"
            ),
        ],
    },
    VariableCopyGroup {
        title: p("Damage taken this round", "本回合承伤"),
        variables: &[
            variable!(
                "Value",
                "数值",
                "round_damage_taken",
                "Total damage taken so far this round. Resets next round.",
                "本回合到现在一共受到多少伤害。下一回合会清零。"
            ),
            variable!(
                "Show when",
                "显示条件",
                "has_round_damage_taken",
                "Show this content once the round has started; 0 is still shown when you take no damage.",
                "回合开始后显示这段内容；没有受伤也会显示 0。"
            ),
        ],
    },
    VariableCopyGroup {
        title: p("Highest DPS", "本局最高 DPS"),
        variables: &[
            variable!(
                "Value",
                "数值",
                "max_dps",
                "Your highest DPS since entering Ecliptica. Resets when you leave.",
                "这次进入 Ecliptica 后的最高 DPS；离开后重置。"
            ),
            variable!(
                "Show when",
                "显示条件",
                "has_max_dps",
                "Show this content when a highest DPS value is available.",
                "有最高 DPS 时显示这段内容。"
            ),
        ],
    },
    VariableCopyGroup {
        title: p("Current combat", "当前战斗"),
        variables: &[
            variable!(
                "Text",
                "文本",
                "boss_lock",
                "Name of the player currently targeted by the Boss; empty when unknown.",
                "Boss 当前锁定的玩家名；还不知道时为空。"
            ),
            variable!(
                "Text",
                "文本",
                "boss",
                "Current Boss name; empty in the lobby or before detection.",
                "当前 Boss 的名字。大厅或还没识别到时为空。"
            ),
        ],
    },
    VariableCopyGroup {
        title: p("Alerts", "战斗提醒"),
        variables: &[
            variable!(
                "Show when",
                "显示条件",
                "is_self_boss_locked",
                "Show this content when the Boss targets you. Enter your VRChat name first.",
                "Boss 锁定你时显示这段内容；需要先填写你的 VRChat 名字。"
            ),
            variable!(
                "Show when",
                "显示条件",
                "rapid_damage_danger",
                "Show this content when you lose a lot of health in a short time.",
                "短时间内掉了很多血时显示这段内容。"
            ),
            variable!(
                "Show when",
                "显示条件",
                "no_dps_for_10s",
                "Show this content when you have not dealt damage for a while.",
                "战斗中有一阵子没打出伤害时显示这段内容。"
            ),
            variable!(
                "Show when",
                "显示条件",
                "no_wasd_for_10s",
                "Show this content when you have not moved for a while.",
                "有一阵子没移动时显示这段内容。"
            ),
        ],
    },
];

pub const REPORT_VARIABLE_GROUPS: &[VariableCopyGroup] = &[
    VariableCopyGroup {
        title: p("Game progress", "游戏进度"),
        variables: &[
            variable!(
                "Show when",
                "显示条件",
                "has_step_estimate",
                "Show this content when the remaining rounds to Jim can be estimated.",
                "能算出距离 Jim 还有几回合时显示这段内容。"
            ),
            variable!(
                "Value",
                "数值",
                "current_step",
                "Estimated round number for the combat that just ended.",
                "预计刚打完的是本局第几回合。"
            ),
            variable!(
                "Value",
                "数值",
                "until_boss_step",
                "Estimated rounds until Jim; 0 means the next combat is expected to be Jim.",
                "预计还要打几回合才到 Jim。0 表示下一战预计就是 Jim。"
            ),
        ],
    },
    VariableCopyGroup {
        title: p("Round duration", "回合用时"),
        variables: &[
            variable!(
                "Value",
                "数值",
                "round_duration",
                "Time from entering combat until returning to the lobby.",
                "从进入战斗到回到大厅，一共用了多久。"
            ),
            variable!(
                "Show when",
                "显示条件",
                "has_round_duration",
                "Show this content when the round duration is available.",
                "有回合用时时显示这段内容。"
            ),
        ],
    },
    VariableCopyGroup {
        title: p("Average DPS", "平均 DPS"),
        variables: &[
            variable!(
                "Value",
                "数值",
                "round_report_avg_dps",
                "Average DPS for the round that just ended.",
                "这一回合的平均 DPS。"
            ),
            variable!(
                "Show when",
                "显示条件",
                "has_round_report_avg_dps",
                "Show this content when average DPS is available.",
                "有平均 DPS 时显示这段内容。"
            ),
        ],
    },
    VariableCopyGroup {
        title: p("Highest DPS", "最高 DPS"),
        variables: &[
            variable!(
                "Value",
                "数值",
                "round_max_dps",
                "Highest DPS in the round that just ended.",
                "这一回合的最高 DPS。"
            ),
            variable!(
                "Show when",
                "显示条件",
                "has_round_max_dps",
                "Show this content when highest DPS is available.",
                "有最高 DPS 时显示这段内容。"
            ),
        ],
    },
    VariableCopyGroup {
        title: p("Active DPS", "持续输出 DPS"),
        variables: &[
            variable!(
                "Value",
                "数值",
                "round_report_effective_dps",
                "Average DPS while attacking; walking and waiting do not lower it.",
                "只看你攻击时的平均 DPS，走路和等待不算。"
            ),
            variable!(
                "Show when",
                "显示条件",
                "has_round_report_effective_dps",
                "Show this content when active DPS is available.",
                "有持续输出 DPS 时显示这段内容。"
            ),
        ],
    },
    VariableCopyGroup {
        title: p("Best average DPS", "最佳平均 DPS"),
        variables: &[
            variable!(
                "Value",
                "数值",
                "round_report_burst_10s",
                "Best average DPS in the round that just ended.",
                "这一回合表现最好的一段平均 DPS。"
            ),
            variable!(
                "Show when",
                "显示条件",
                "has_round_report_burst_10s",
                "Show this content when best average DPS is available.",
                "有最佳平均 DPS 时显示这段内容。"
            ),
        ],
    },
    VariableCopyGroup {
        title: p("Active DPS change", "持续输出 DPS 变化"),
        variables: &[
            variable!(
                "Value",
                "数值",
                "dps_growth_rate",
                "Change in active DPS from the previous round. The template receives a number without the percent sign.",
                "持续输出 DPS 比上一回合高了或低了多少；填入模板的是数字，不带百分号。"
            ),
            variable!(
                "Show when",
                "显示条件",
                "has_dps_growth_rate",
                "Show this content when there is a previous round to compare.",
                "有上一回合可比较时显示这段内容。"
            ),
        ],
    },
    VariableCopyGroup {
        title: p("Longest standstill", "最长站桩"),
        variables: &[
            variable!(
                "Value",
                "数值",
                "round_longest_standstill",
                "Longest standstill in the round. The template receives seconds without a unit.",
                "这一回合的最长站桩时间；填入模板的是秒数，不带单位。"
            ),
            variable!(
                "Show when",
                "显示条件",
                "has_round_longest_standstill",
                "Show this content when standstill data is available.",
                "有站桩数据时显示这段内容。"
            ),
        ],
    },
    VariableCopyGroup {
        title: p("Round damage", "伤害统计"),
        variables: &[
            variable!(
                "Value",
                "数值",
                "round_total_damage",
                "Total damage dealt in the round that just ended.",
                "这一回合一共打了多少伤害。"
            ),
            variable!(
                "Value",
                "数值",
                "round_report_damage_taken",
                "Total damage taken in the round that just ended.",
                "这一回合一共受到多少伤害。"
            ),
        ],
    },
];

pub const TEMPLATE_SYNTAX_EXAMPLES: &[SyntaxCopy] = &[
    SyntaxCopy {
        title: p("1 · Insert data", "1 · 插入数据"),
        code: p("Current DPS: {{latest_dps}}", "当前 DPS: {{latest_dps}}"),
    },
    SyntaxCopy {
        title: p("2 · Show only when needed", "2 · 需要时才显示"),
        code: p(
            "{{#if has_latest_dps}}\nCurrent DPS: {{latest_dps}}\n{{/if}}",
            "{{#if has_latest_dps}}\n当前 DPS: {{latest_dps}}\n{{/if}}",
        ),
    },
    SyntaxCopy {
        title: p("3 · Choose between two messages", "3 · 两段话选一段"),
        code: p(
            "{{#if is_self_boss_locked}}\n⚠ Boss is targeting me\n{{else}}\nBoss is not targeting me\n{{/if}}",
            "{{#if is_self_boss_locked}}\n⚠ Boss 正在锁定我\n{{else}}\nBoss 当前没有锁定我\n{{/if}}",
        ),
    },
    SyntaxCopy {
        title: p("4 · Compare or combine conditions", "4 · 比较或组合条件"),
        code: p(
            "{{#if (gt round_damage_taken 50)}}Damage taken exceeds 50{{/if}}\n{{#if (or rapid_damage_danger no_dps_for_10s)}}⚠ Check combat status{{/if}}",
            "{{#if (gt round_damage_taken 50)}}承伤超过 50{{/if}}\n{{#if (or rapid_damage_danger no_dps_for_10s)}}⚠ 注意战斗状态{{/if}}",
        ),
    },
];

pub fn format_seconds_pattern(language: Language, seconds: &str) -> String {
    text::EVERY_SECONDS
        .get(language)
        .replace("{seconds}", seconds)
}

pub fn format_pattern(
    pair: TextPair,
    language: Language,
    replacements: &[(&str, String)],
) -> String {
    replacements
        .iter()
        .fold(pair.get(language).to_owned(), |text, (key, value)| {
            text.replace(&format!("{{{key}}}"), value)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chinese_locale_variants_select_chinese() {
        assert_eq!(Language::from_locale("zh-CN"), Language::Chinese);
        assert_eq!(Language::from_locale("zh_Hant_TW"), Language::Chinese);
    }

    #[test]
    fn non_chinese_and_invalid_locales_select_english() {
        assert_eq!(Language::from_locale("en-US"), Language::English);
        assert_eq!(Language::from_locale("ja_JP"), Language::English);
        assert_eq!(Language::from_locale("ko-KR"), Language::English);
        assert_eq!(Language::from_locale("zhongwen"), Language::English);
        assert_eq!(Language::from_locale(""), Language::English);
    }

    #[test]
    fn paired_text_selects_the_requested_language() {
        let pair = TextPair::new("Overview", "首页");
        assert_eq!(pair.get(Language::English), "Overview");
        assert_eq!(pair.get(Language::Chinese), "首页");
    }

    #[test]
    fn central_catalog_contains_no_empty_translation_halves() {
        let source = include_str!("i18n.rs");
        assert!(!source.contains("p(\"\","));
        assert!(!source.contains(", \"\")"));
        assert!(!source.contains("pair!(\"\""));
    }

    #[test]
    fn heart_rate_user_copy_avoids_internal_network_details() {
        let user_copy = [
            text::ENABLE_HEART_RATE_HINT,
            text::HEART_RATE_SERVER_FAILED,
            text::HEART_RATE_NO_PORT,
            text::HEART_RATE_CONNECTED,
            text::HEART_RATE_DISCONNECTED,
            text::HEART_RATE_WAITING,
            text::HEART_RATE_VARIABLE_OFFLINE,
        ];

        for copy in user_copy {
            for language in Language::ALL {
                let value = copy.get(language).to_ascii_lowercase();
                assert!(
                    ![
                        "127.0.0.1",
                        "49670",
                        "http",
                        "protocol",
                        "host",
                        "端口",
                        "协议",
                        "主机"
                    ]
                    .iter()
                    .any(|technical_term| value.contains(technical_term)),
                    "heart-rate copy exposes an internal detail: {value}"
                );
                assert!(
                    !value
                        .split(|character: char| !character.is_ascii_alphabetic())
                        .any(|word| matches!(word, "port" | "ports")),
                    "heart-rate copy exposes an internal port detail: {value}"
                );
            }
        }
    }
}
