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
    pub description: TextPair,
    pub code: TextPair,
}

#[derive(Debug, Clone, Copy)]
pub struct HelpTipCopy {
    pub question: TextPair,
    pub answer: TextPair,
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

    pair!(OVERVIEW, "Overview", "运行总览");
    pair!(OSC_MESSAGES, "OSC Messages", "OSC 消息");
    pair!(PLAYER_ALERTS, "Player Alerts", "玩家提醒");
    pair!(OVERLAY, "Overlay", "悬浮窗");
    pair!(SYSTEM_LOGS, "System Logs", "系统日志");
    pair!(WORKSPACE, "WORKSPACE", "工作区");
    pair!(
        SAVE_SUCCESS,
        "Settings saved; OSC will apply them on the next send cycle",
        "设置已保存；OSC 将在下一发送周期应用"
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
    pair!(SAVE_AND_APPLY, "Save and apply", "保存修改并应用");
    pair!(SAVED, "Saved", "当前已保存");
    pair!(
        SAVE_AND_APPLY_HINT,
        "Save all changes and apply now",
        "保存全部修改并立即应用"
    );
    pair!(
        NO_CHANGES_TO_SAVE,
        "There are no changes to save",
        "当前没有需要保存的修改"
    );
    pair!(VIEW_FULL_ERROR, "View full error", "查看完整错误");
    pair!(ECLIPTICA_DETECTED, "Ecliptica detected", "Ecliptica 已识别");
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
        "This will overwrite all unsaved changes. You will still need to save and apply to write the configuration.",
        "这会覆盖当前管理页中的所有未保存修改。确认后仍需点击“保存并应用”才会写入配置。"
    );
    pair!(CANCEL, "Cancel", "取消");
    pair!(RESTORE, "Restore", "确认恢复");
    pair!(
        DEFAULTS_RESTORED,
        "Defaults restored; save to apply",
        "已恢复默认值，保存后生效"
    );
    pair!(TEMPLATE_SYNTAX_HELP, "Template syntax help", "模板语法帮助");
    pair!(
        TEMPLATE_SYNTAX_HELP_DESCRIPTION,
        "Use the examples below to insert variables and combine conditions.",
        "从插入变量到组合条件，按下面的例子替换变量名即可。"
    );
    pair!(
        SAVE_ERROR_DESCRIPTION,
        "The complete error is preserved below and can be scrolled without truncation.",
        "下面保留了完整错误信息，可滚动查看，不会截断。"
    );
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
    pair!(
        OVERVIEW_SUBTITLE,
        "Monitor analyzer, Boss, and OSC status in real time",
        "实时查看分析器、Boss 与 OSC 状态"
    );
    pair!(UNPIN_WINDOW, "Unpin window", "取消固定窗口");
    pair!(PIN_WINDOW, "Pin window", "固定窗口");
    pair!(
        UNPIN_WINDOW_HINT,
        "Return to normal window level",
        "点击恢复普通窗口层级"
    );
    pair!(
        PIN_WINDOW_HINT,
        "Keep the main window above other windows",
        "让主窗口始终显示在其他窗口上方"
    );
    pair!(LIVE_DPS, "Live DPS", "实时 DPS");
    pair!(AVERAGE_DPS_30S, "30-second average DPS", "30 秒平均 DPS");
    pair!(ROUND_EFFECTIVE_DPS, "Round effective DPS", "回合有效 DPS");
    pair!(ROUND_BURST_10S, "Round 10-second burst", "回合 10 秒爆发");
    pair!(ROUND_DAMAGE_TAKEN, "Round damage taken", "回合承伤");
    pair!(BOSS_LOCK, "BOSS LOCK", "BOSS 锁定");
    pair!(SESSION_DPS_CHART, "Session DPS chart", "本局 DPS 曲线");
    pair!(
        SESSION_DPS_CHART_DESCRIPTION,
        "View personal DPS by round; the horizontal axis is elapsed session time, not system time",
        "按回合查看个人 DPS；横轴是进入本局后的经过时间，不使用系统时间"
    );
    pair!(
        PREVIOUS_ROUND_REPORT,
        "Previous round report",
        "上一回合战报"
    );
    pair!(
        PREVIOUS_ROUND_REPORT_DESCRIPTION,
        "Kept during upgrades and cleared when the next combat begins",
        "升级阶段保留，进入下一战斗场景后自动清除"
    );
    pair!(DURATION, "Duration", "回合用时");
    pair!(TOTAL_DAMAGE, "Total damage", "总输出");
    pair!(EFFECTIVE_DPS, "Effective DPS", "有效 DPS");
    pair!(BURST_10S, "10-second burst", "10 秒爆发");
    pair!(
        EFFECTIVE_DPS_GROWTH,
        "Effective DPS growth",
        "有效 DPS 增长率"
    );
    pair!(DAMAGE_TAKEN, "Damage taken", "承伤");
    pair!(LONGEST_STANDSTILL, "Longest standstill", "最长站桩");
    pair!(DATA_SOURCE, "Data source", "数据源");
    pair!(
        DATA_SOURCE_DESCRIPTION,
        "Current combat context and log source",
        "当前战斗上下文与日志来源"
    );
    pair!(CURRENT_BOSS, "Current Boss", "当前 Boss");
    pair!(NO_CURRENT_BOSS, "No current Boss", "当前无 Boss");
    pair!(CURRENT_PHASE, "Current phase", "当前阶段");
    pair!(SYNC_STATUS, "Sync status", "同步状态");
    pair!(
        JOINED_MID_SESSION,
        "Joined mid-session · waiting for next round",
        "中途加入 · 等待下一回合"
    );
    pair!(
        ROUND_DATA_AVAILABLE,
        "Current round data available",
        "本回合数据可用"
    );
    pair!(SOUND_SOFT, "Soft", "柔和");
    pair!(SOUND_CRISP, "Crisp", "清脆");
    pair!(SOUND_PROMINENT, "Prominent", "强提醒");
    pair!(PHASE_OUTSIDE, "Outside Ecliptica", "未进入 Ecliptica");
    pair!(PHASE_SYNCING, "Syncing room progress", "正在同步房间进度");
    pair!(PHASE_LOBBY, "Lobby / Upgrade phase", "大厅 / 升级阶段");
    pair!(PHASE_COMBAT, "Round in progress", "回合战斗中");
    pair!(
        OSC_MESSAGES_SUBTITLE,
        "Configure the send interval, destination, and Chatbox templates",
        "配置发送频率、目标地址与 Chatbox 模板"
    );
    pair!(SEND_SETTINGS, "Send settings", "发送设置");
    pair!(
        SEND_SETTINGS_DESCRIPTION,
        "VRChat listens at 127.0.0.1:9000 by default",
        "VRChat 默认接收地址为 127.0.0.1:9000"
    );
    pair!(ENABLE_OSC, "Enable OSC broadcast", "启用 OSC 广播");
    pair!(SEND_INTERVAL, "Send interval", "发送频率");
    pair!(TARGET_ADDRESS, "Destination", "目标地址");
    pair!(EVERY_SECONDS, "Every {seconds} seconds", "每 {seconds} 秒");
    pair!(
        PLAYER_ALERTS_SUBTITLE,
        "Play different sounds when a Boss targets you and when it switches to another player",
        "Boss 锁定自己及从自己转移到其他玩家时播放不同提示音"
    );
    pair!(PLAYER_IDENTITY, "Player identity", "玩家身份");
    pair!(
        PLAYER_IDENTITY_DESCRIPTION,
        "Visual alerts and sounds are disabled while the name is empty",
        "名字为空时不会触发醒目状态和提示音"
    );
    pair!(
        DISPLAY_NAME_PLACEHOLDER,
        "Enter your VRChat display name",
        "输入你的 VRChat 显示名称"
    );
    pair!(
        VRCHAT_DISPLAY_NAME,
        "VRChat Display Name",
        "VRChat 显示名称"
    );
    pair!(
        DISPLAY_NAME_NORMALIZATION,
        "Comparison trims whitespace, applies Unicode NFKC normalization, and ignores case.",
        "比较时会去除首尾空格、执行 Unicode NFKC 规范化，并忽略大小写。"
    );
    pair!(ALERT_SOUNDS, "Alert sounds", "提示音");
    pair!(
        ALERT_SOUNDS_DESCRIPTION,
        "The lock sound has a 5-second cooldown; the release sound only plays when the same Boss explicitly changes targets",
        "锁定音有 5 秒冷却；脱离音仅在同一 Boss 明确转移锁定时触发"
    );
    pair!(VOLUME, "Volume", "音量");
    pair!(ALERT_VOLUME, "Alert volume", "提示音音量");
    pair!(LOCK_SOUND, "Targeted sound", "被锁音");
    pair!(RELEASE_SOUND, "Released sound", "脱离音");
    pair!(PREVIEW_SOUND, "Preview", "试听");
    pair!(
        OVERLAY_SUBTITLE,
        "Control the floating information card's position and interaction",
        "控制悬浮信息卡的位置与交互方式"
    );
    pair!(WINDOW_BEHAVIOR, "Window behavior", "窗口行为");
    pair!(
        WINDOW_BEHAVIOR_DESCRIPTION,
        "The Overlay is always on top. Enable dragging to interact with it; disable it to pass mouse input through to windows below",
        "Overlay 始终置顶；开启后可拖动，关闭后鼠标操作会穿透到下层窗口"
    );
    pair!(DRAGGABLE, "Draggable", "可拖动");
    pair!(
        DRAG_OVERLAY_HINT,
        "Drag anywhere on the Overlay to move it.",
        "拖住 Overlay 的任意位置即可移动窗口。"
    );
    pair!(OVERLAY_SIZE, "Overlay size", "Overlay 大小");
    pair!(SCREEN_POSITION, "Screen position", "屏幕位置");
    pair!(
        SCREEN_POSITION_DESCRIPTION,
        "Uses the top-left of the primary display as the origin and moves immediately after saving",
        "以主显示器左上角为原点，保存后立即移动"
    );
    pair!(HORIZONTAL_POSITION, "Horizontal position X", "水平位置 X");
    pair!(VERTICAL_POSITION, "Vertical position Y", "垂直位置 Y");
    pair!(PIXELS_SUFFIX, " px", " 像素");
    pair!(
        SYSTEM_LOGS_SUBTITLE,
        "Recent log-reader, OSC, and audio-device status",
        "最近的日志读取、OSC 与声音设备状态"
    );
    pair!(
        DEVELOPER_LOGS_HINT,
        "Open protocol compatibility and other specialized diagnostics",
        "打开协议兼容性等专用诊断"
    );
    pair!(EVENT_STREAM, "Event stream", "事件流");
    pair!(NO_SYSTEM_EVENTS, "No system events", "暂无系统事件");
    pair!(INFO, "Info", "信息");
    pair!(WARNING, "Warning", "警告");
    pair!(ERROR, "Error", "错误");
    pair!(
        NORMAL_MESSAGE_TEMPLATE,
        "Regular message template",
        "普通消息模板"
    );
    pair!(
        NORMAL_MESSAGE_TEMPLATE_DESCRIPTION,
        "Used during combat; switch and rename 3 presets, with names, content, and selection saved",
        "战斗中使用；3 个预设可切换、改名，保存后会记住名称、内容与当前选择"
    );
    pair!(TEMPLATE_PRESET, "Template preset", "模板预设");
    pair!(PRESET_NAME, "Preset name", "预设名称");
    pair!(PRESET_FALLBACK, "Preset {index}", "预设 {index}");
    pair!(
        PRESET_SWITCHED,
        "Switched to “{name}”; save to apply",
        "已切换到「{name}」，保存后应用"
    );
    pair!(
        PRESET_NAME_HINT,
        "Up to {max} characters; saved persistently",
        "最多 {max} 个字符；保存后持久化"
    );
    pair!(
        LIVE_VARIABLES_HINT,
        "Click a variable to copy it. “Condition” variables control whether a block of text is shown.",
        "点击变量即可复制。“条件”变量用来控制一段文字是否显示。"
    );
    pair!(
        ROUND_REPORT_TEMPLATE,
        "Round report template",
        "回合战报模板"
    );
    pair!(
        ROUND_REPORT_TEMPLATE_DESCRIPTION,
        "Used after returning to the upgrade lobby; 3 presets can be switched, renamed, and saved independently",
        "返回升级大厅后使用；3 个预设可独立切换、改名并持久保存"
    );
    pair!(REPORT_PRESET, "Report preset", "战报预设");
    pair!(
        REPORT_VARIABLES_HINT,
        "Click a variable to copy it. These variables are only used after a round ends.",
        "点击变量即可复制。这里的变量只在一回合结束后使用。"
    );
    pair!(LIVE_PREVIEW, "Live preview", "实时预览");
    pair!(
        LIVE_PREVIEW_DESCRIPTION,
        "Switch the simulated state to see the final text after VRChat Chatbox limits are applied",
        "切换模拟状态，查看按 VRChat Chatbox 限制处理后的实际文本"
    );
    pair!(SIMULATED_STATE, "Simulated state", "模拟状态");
    pair!(PREVIEW_NORMAL, "Normal", "普通");
    pair!(PREVIEW_MID_SESSION, "Joined mid-session", "中途加入");
    pair!(PREVIEW_ROUND_REPORT, "Round report", "回合战报");
    pair!(
        EMPTY_MESSAGE,
        "The current message is empty and will not be sent",
        "当前消息为空，不会发送"
    );
    pair!(TEMPLATE_ERROR, "Template error", "模板错误");
    pair!(
        CHART_WAITING_FIRST_SECOND,
        "Waiting for the first second of data…",
        "正在等待第一秒数据…"
    );
    pair!(
        CHART_ENTER_ECLIPTICA,
        "Enter Ecliptica to begin recording this session",
        "进入 Ecliptica 后开始记录本局曲线"
    );
    pair!(
        CHART_ZERO_DESCRIPTION,
        "Lobby and no-damage periods are recorded as 0; the last session remains after leaving the room.",
        "大厅和无输出时段也会记录为 0，离开房间后保留最后一局曲线。"
    );
    pair!(
        CHART_WAITING_ROUND,
        "Waiting for a round to begin…",
        "等待回合开始…"
    );
    pair!(
        CHART_WAITING_ROUND_DESCRIPTION,
        "The current round's DPS chart will appear here after combat begins.",
        "进入战斗后会在这里显示当前回合的 DPS 曲线。"
    );
    pair!(
        CHART_WAITING_DATA,
        "Waiting for current round data…",
        "正在等待当前回合数据…"
    );
    pair!(
        CHART_CURRENT_ESTIMATED_ROUND,
        "Estimated current round {step}",
        "当前预估第 {step} 回合"
    );
    pair!(
        CHART_FINISHED_ESTIMATED_ROUND,
        "Estimated completed round {step}",
        "刚结束的预估第 {step} 回合"
    );
    pair!(CHART_CURRENT_ROUND, "Current round", "当前回合");
    pair!(CHART_FINISHED_ROUND, "Completed round", "刚结束的回合");
    pair!(DPS_TREND, "DPS trend", "DPS 趋势");
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
    pair!(LATEST, "Latest", "最新");
    pair!(EFFECTIVE, "Effective", "有效");
    pair!(BURST_10S_SHORT, "10s", "10秒");
    pair!(DAMAGE_TAKEN_SHORT, "Taken", "承伤");
    pair!(
        ROUND_DAMAGE_TAKEN_TOTAL,
        "Total round damage taken",
        "回合承伤总量"
    );
    pair!(EXACT_VALUE, "Exact value", "精确值");
    pair!(LOG_FILE, "Log file", "日志文件");
    pair!(OPEN_FOLDER, "Open folder", "打开目录");
    pair!(SEARCHING, "Searching…", "正在查找…");
    pair!(
        LOG_NOT_FOUND,
        "No log file has been detected",
        "尚未识别到日志文件"
    );
    pair!(
        LOG_MISSING,
        "The log file or its folder no longer exists",
        "日志文件或所在目录已不存在"
    );
    pair!(
        OPEN_LOG_FOLDER_FINDER,
        "Open the log folder in Finder",
        "在 Finder 中打开日志所在目录"
    );
    pair!(
        OPEN_LOG_FOLDER_EXPLORER,
        "Open the log folder in File Explorer",
        "在文件资源管理器中打开日志所在目录"
    );
    pair!(
        OPEN_LOG_FOLDER_MANAGER,
        "Open the log folder in the file manager",
        "在文件管理器中打开日志所在目录"
    );
    pair!(
        MAX_LOG_ROWS,
        "Keeps up to {max} entries and merges adjacent duplicates",
        "最多保留 {max} 条，相邻重复消息自动合并"
    );
    pair!(
        SAVE_ERROR_GUIDANCE,
        "Fix the indicated location and cause below, then save the template again.",
        "请根据下面的具体位置和原因修改模板，然后重新保存。"
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
    pair!(ROLE_CONDITION, "Condition", "条件");
    pair!(ROLE_VALUE, "Value", "数值");
    pair!(ROLE_TEXT, "Text", "文本");
    pair!(ROLE_STATUS, "Status", "状态");
    pair!(ROLE_JUDGMENT, "Check", "判断");
    pair!(ROLE_DISPLAY, "Display", "显示");
    pair!(OPENED_LOG_FOLDER, "Opened log folder", "已打开日志所在目录");
    pair!(
        OPEN_LOG_FOLDER_FAILED,
        "Failed to open log folder",
        "打开日志所在目录失败"
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
        "Configuration saved atomically; OSC is waiting for the next send cycle",
        "配置已原子保存，OSC 正等待下一发送周期"
    );
    pair!(ROUND_TICK, "{step} rounds", "{step}轮");
    pair!(COMPACT_TEN_THOUSAND, "K", "万");
    pair!(COMPACT_HUNDRED_MILLION, "M", "亿");
    pair!(STATUS_SEARCHING, "SEARCHING", "查找中");
    pair!(STATUS_RECOVERING, "RECOVERING", "恢复中");
    pair!(STATUS_LIVE, "LIVE", "正常");
    pair!(STATUS_STALE, "STALE", "等待数据");
    pair!(STATUS_ERROR, "ERROR", "错误");
    pair!(
        AUDIO_INIT_FAILED,
        "Failed to initialize audio device",
        "声音设备初始化失败"
    );
    pair!(
        AUDIO_PLAYBACK_FAILED,
        "Failed to play alert sound",
        "提示音播放失败"
    );
    pair!(
        WASD_INIT_FAILED,
        "Failed to initialize WASD event listener; metric unavailable",
        "WASD 事件监听初始化失败，指标不可用"
    );
    pair!(
        WASD_KEYBOARD_INIT_FAILED,
        "Failed to initialize WASD keyboard listener; metric unavailable",
        "WASD 键盘监听初始化失败，指标不可用"
    );
    pair!(
        WASD_INTERRUPTED,
        "WASD keyboard listener stopped unexpectedly; metric disabled",
        "WASD 键盘监听意外中断，指标已停用"
    );
    pair!(
        LOG_DISCOVERY_FAILED,
        "Failed to find VRChat logs",
        "查找 VRChat 日志失败"
    );
    pair!(
        LOG_REPLACED,
        "The log at the same path was replaced; state was cleared and restored from the new file",
        "日志在相同路径被替换，已清空状态并从新文件恢复"
    );
    pair!(
        LOG_ID_FAILED,
        "Failed to read log file identity",
        "读取日志文件标识失败"
    );
    pair!(
        LOG_TRUNCATED,
        "The log was truncated; combat state was cleared and restored from the beginning",
        "日志被截断，已清空战斗状态并从头恢复"
    );
    pair!(
        LOG_READ_FAILED,
        "Failed to read log; searching again",
        "读取日志失败，准备重新查找"
    );
    pair!(
        LOG_FOUND,
        "Log found; quietly restoring current state",
        "发现日志，正在静默恢复当前状态"
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
        "Log reading is healthy; OSC broadcasting is enabled",
        "日志读取正常，OSC 广播已允许"
    );
    pair!(
        LOG_STALE,
        "No new log content; OSC is paused while waiting for data",
        "日志暂时无新增内容，OSC 已暂停并等待新数据"
    );
    pair!(
        LOG_SEARCHING,
        "No VRChat log found; retrying",
        "未找到 VRChat 日志，将继续重试"
    );
    pair!(
        LOG_RECOVERING,
        "Restoring state from the log; historical data will not trigger sounds or OSC",
        "正在从日志恢复状态，历史数据不会触发声音或 OSC"
    );
    pair!(
        LOG_ERROR,
        "Log reading failed; OSC is paused",
        "日志读取发生错误，OSC 已暂停"
    );
    pair!(
        OSC_INIT_FAILED,
        "Failed to initialize OSC UDP",
        "OSC UDP 初始化失败"
    );
    pair!(
        OSC_STATE_PACKET_SUBMITTED,
        "OSC state-change packet submitted; older pending messages were discarded",
        "OSC 状态切换包已提交，旧待发送消息已丢弃"
    );
    pair!(OSC_SEND_FAILED, "OSC send failed", "OSC 发送失败");
    pair!(
        SINGLE_INSTANCE_FAILED,
        "Failed to create the single-instance lock",
        "创建单实例锁失败"
    );
    pair!(
        ALREADY_RUNNING,
        "Ecliptica Data Analyzer is already running",
        "Ecliptica Data Analyzer 已经在运行"
    );
    pair!(
        EXIT_HANDLER_FAILED,
        "Failed to register the system exit handler",
        "注册系统退出信号处理失败"
    );
    pair!(
        VOLUME_INVALID,
        "Alert volume must be between 0 and 1",
        "提示音量必须在 0 到 1 之间"
    );
    pair!(
        OVERLAY_SCALE_INVALID,
        "Overlay scale must be between 0.5× and 3×",
        "Overlay 缩放必须在 0.5 到 3 倍之间"
    );
    pair!(
        STALE_TIME_INVALID,
        "Log stale time must be between 2 and 300 seconds",
        "日志过期时间必须在 2 到 300 秒之间"
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
        "Cannot determine the system configuration directory",
        "无法确定系统配置目录"
    );
    pair!(
        CONFIG_READ_FAILED,
        "Failed to read configuration: {path}",
        "读取配置失败：{path}"
    );
    pair!(
        CONFIG_JSON_CORRUPT,
        "Configuration JSON is corrupted",
        "配置 JSON 已损坏"
    );
    pair!(
        CONFIG_BACKUP_FAILED,
        "The corrupted configuration could not be backed up to {path}",
        "配置损坏且无法备份到 {path}"
    );
    pair!(
        CONFIG_RECOVERED,
        "The configuration was corrupted and defaults were restored. The original file is at {path}",
        "配置损坏，已恢复默认值。原文件保存在 {path}"
    );
    pair!(
        CONFIG_DIR_CREATE_FAILED,
        "Failed to create configuration directory: {path}",
        "创建配置目录失败：{path}"
    );
    pair!(
        CONFIG_TEMP_CREATE_FAILED,
        "Failed to create a temporary configuration file",
        "创建配置临时文件失败"
    );
    pair!(
        CONFIG_SERIALIZE_FAILED,
        "Failed to serialize configuration",
        "序列化配置失败"
    );
    pair!(
        CONFIG_TEMP_SYNC_FAILED,
        "Failed to sync the temporary configuration file",
        "同步配置临时文件失败"
    );
    pair!(
        CONFIG_REPLACE_FAILED,
        "Failed to atomically replace configuration: {path}",
        "原子替换配置失败：{path}"
    );
    pair!(
        USERPROFILE_MISSING,
        "USERPROFILE is not set",
        "USERPROFILE 未设置"
    );
    pair!(
        CONFIG_VERSION_UNSUPPORTED,
        "Configuration version {version} is newer than supported version {supported}",
        "配置版本 {version} 高于本程序支持的版本 {supported}"
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
        "Log path has no parent folder",
        "日志路径没有所在目录"
    );
    pair!(
        LOG_FOLDER_MISSING,
        "Log folder does not exist",
        "日志所在目录不存在"
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
        "A background worker exited unexpectedly",
        "后台工作线程异常退出"
    );
    pair!(HELP_MOST_USED, "Most-used syntax", "最常用的写法");
    pair!(
        HELP_MOST_USED_DESCRIPTION,
        "Start with variables and if. Use the advanced syntax below when you need more flexible conditions.",
        "先学会显示变量和 if 就够用了。需要更灵活的判断时，再看下面的进阶写法。"
    );
    pair!(HELP_MORE_SYNTAX, "More available syntax", "更多可用语法");
    pair!(
        HELP_MORE_SYNTAX_DESCRIPTION,
        "These forms are supported now, not only if.",
        "这些语法当前已经支持，不只是 if。"
    );
    pair!(
        HELP_UNUSED_SYNTAX,
        "Syntax not currently useful",
        "目前用不上的语法"
    );
    pair!(
        HELP_UNUSED_SYNTAX_DESCRIPTION,
        "The engine also supports each, with, and lookup, but templates currently receive only scalar values and text, not lists or complex objects.",
        "引擎也有 each、with、lookup，但当前模板拿到的都是单个数值或文字，没有列表和复杂对象，因此这里暂时没有实际用途。"
    );
    pair!(HELP_FAQ, "Common questions", "常见问题");
    pair!(
        HELP_CLOSE,
        "Close with × in the top-right, or click outside the dialog.",
        "点击弹窗右上角 ×，或点击弹窗外区域即可关闭。"
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

pub const LIVE_VARIABLE_GROUPS: &[VariableCopyGroup] = &[
    VariableCopyGroup {
        title: p("Latest DPS", "最新 DPS"),
        variables: &[
            variable!(
                "Value",
                "数值",
                "latest_dps",
                "Your DPS in the most recent second. It becomes 0 after several seconds without damage.",
                "你最近一秒打出的 DPS。停止输出几秒后会变成 0。"
            ),
            variable!(
                "Condition",
                "条件",
                "has_latest_dps",
                "Enabled when latest DPS is available to display.",
                "已经有最新 DPS 可显示时开启。"
            ),
        ],
    },
    VariableCopyGroup {
        title: p("30-second average DPS", "30 秒平均 DPS"),
        variables: &[
            variable!(
                "Value",
                "数值",
                "avg_dps",
                "Your average DPS over the last 30 seconds. Displays “-” when unavailable.",
                "你最近 30 秒的平均 DPS。还没有数据时显示“-”。"
            ),
            variable!(
                "Condition",
                "条件",
                "has_avg_dps",
                "Enabled when 30-second average DPS is available.",
                "已经有 30 秒平均 DPS 可显示时开启。"
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
                "Condition",
                "条件",
                "has_round_avg_dps",
                "Enabled when current-round average DPS is available.",
                "有本回合平均 DPS 时开启。可用来隐藏无数据的文字。"
            ),
        ],
    },
    VariableCopyGroup {
        title: p("Round effective DPS", "回合有效 DPS"),
        variables: &[
            variable!(
                "Value",
                "数值",
                "round_effective_dps",
                "DPS while you are actively dealing damage; walking and long waits do not reduce it.",
                "只计算你在持续输出时的 DPS，走路和长时间等待不会拉低它。"
            ),
            variable!(
                "Condition",
                "条件",
                "has_round_effective_dps",
                "Enabled when current-round effective DPS is available.",
                "有本回合有效 DPS 时开启。"
            ),
        ],
    },
    VariableCopyGroup {
        title: p("Round 10-second burst", "回合 10 秒爆发"),
        variables: &[
            variable!(
                "Value",
                "数值",
                "round_burst_10s",
                "Best rolling 10-second average DPS this round. Displays “-” before 10 seconds.",
                "本回合表现最好的连续 10 秒平均 DPS。不满 10 秒时显示“-”。"
            ),
            variable!(
                "Condition",
                "条件",
                "has_round_burst_10s",
                "Enabled after a 10-second burst DPS value is available.",
                "已经打满 10 秒、有爆发 DPS 可显示时开启。"
            ),
        ],
    },
    VariableCopyGroup {
        title: p("Round damage taken", "回合承伤"),
        variables: &[
            variable!(
                "Value",
                "数值",
                "round_damage_taken",
                "Total damage taken so far this round. Resets next round.",
                "本回合到现在一共受到多少伤害。下一回合会清零。"
            ),
            variable!(
                "Condition",
                "条件",
                "has_round_damage_taken",
                "Enabled once this round has combat data; a real 0 is still displayed.",
                "本回合已经有战斗记录时开启。没有受伤也会正常显示 0。"
            ),
        ],
    },
    VariableCopyGroup {
        title: p("Session maximum DPS", "本场最高 DPS"),
        variables: &[
            variable!(
                "Value",
                "数值",
                "max_dps",
                "Highest one-second DPS since entering the room. Cleared when leaving.",
                "这次进入游戏后出现过的最高一秒 DPS。离开房间后清空。"
            ),
            variable!(
                "Condition",
                "条件",
                "has_max_dps",
                "Enabled when a maximum DPS record exists.",
                "已经有最高 DPS 记录时开启。"
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
                "Boss 当前盯着的玩家名。还不知道时为空。"
            ),
            variable!(
                "Text",
                "文本",
                "boss",
                "Current Boss name; empty in the lobby or before detection.",
                "当前 Boss 的名字。大厅或还没识别到时为空。"
            ),
            variable!(
                "Status",
                "状态",
                "status",
                "Whether the analyzer is searching for, reading, or waiting for game data.",
                "分析器当前是否正在找到、读取或等待游戏数据。"
            ),
            variable!(
                "Status",
                "状态",
                "phase",
                "Whether you are outside the game, syncing, in the lobby, or in combat.",
                "当前在游戏外、同步中、大厅还是战斗中。"
            ),
        ],
    },
    VariableCopyGroup {
        title: p("Alert conditions", "提醒开关"),
        variables: &[
            variable!(
                "Condition",
                "条件",
                "is_self_boss_locked",
                "Enabled when the Boss targets you. Your player name must be set first.",
                "Boss 正在盯着你时开启。需要先填写自己的玩家名。"
            ),
            variable!(
                "Condition",
                "条件",
                "rapid_damage_danger",
                "Enabled after taking more than 50 damage in the last 10 seconds.",
                "最近 10 秒受到超过 50 点伤害时开启。"
            ),
            variable!(
                "Condition",
                "条件",
                "no_dps_for_10s",
                "Enabled after dealing no damage for 10 continuous seconds in combat.",
                "战斗中连续 10 秒没有打出伤害时开启。"
            ),
            variable!(
                "Condition",
                "条件",
                "no_wasd_for_10s",
                "Enabled after 10 continuous seconds without pressing W, A, S, or D.",
                "连续 10 秒没有按 W、A、S、D 时开启。"
            ),
            variable!(
                "Condition",
                "条件",
                "waiting_for_next_round",
                "Enabled after joining mid-session while waiting for the next round.",
                "中途加入、正在等下一回合时开启。"
            ),
        ],
    },
];

pub const REPORT_VARIABLE_GROUPS: &[VariableCopyGroup] = &[
    VariableCopyGroup {
        title: p("Jim round estimate", "Jim 回合估计"),
        variables: &[
            variable!(
                "Condition",
                "条件",
                "has_step_estimate",
                "Enabled when the app can reliably estimate the rounds remaining until Jim.",
                "软件有把握估计 Jim 还剩几回合时开启；信息不够时不会显示。"
            ),
            variable!(
                "Value",
                "数值",
                "current_step",
                "Estimated session round number of the combat that just ended.",
                "刚打完的战斗预计是本局第几回合。中途加入也会继续修正。"
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
        title: p("Total round duration", "回合总用时"),
        variables: &[
            variable!(
                "Value",
                "数值",
                "round_duration",
                "Time from entering combat until returning to the lobby.",
                "从进入战斗到回到大厅，一共用了多久。"
            ),
            variable!(
                "Condition",
                "条件",
                "has_round_duration",
                "Enabled when a complete round duration is available.",
                "有完整回合时间可显示时开启。"
            ),
        ],
    },
    VariableCopyGroup {
        title: p("Combat duration", "实际战斗用时"),
        variables: &[
            variable!(
                "Value",
                "数值",
                "round_combat_duration",
                "Time from your first damage until returning to the lobby.",
                "从你第一次打出伤害到回到大厅，用了多久。"
            ),
            variable!(
                "Condition",
                "条件",
                "has_round_combat_duration",
                "Enabled when the round contains damage and combat duration is available.",
                "本回合打出过伤害、有战斗时间可显示时开启。"
            ),
        ],
    },
    VariableCopyGroup {
        title: p("Round average DPS", "回合平均 DPS"),
        variables: &[
            variable!(
                "Value",
                "数值",
                "round_report_avg_dps",
                "Average DPS for the completed round.",
                "刚结束这回合的平均 DPS。"
            ),
            variable!(
                "Condition",
                "条件",
                "has_round_report_avg_dps",
                "Enabled when average DPS is available for this round.",
                "本回合有平均 DPS 可显示时开启。"
            ),
        ],
    },
    VariableCopyGroup {
        title: p("Round maximum DPS", "回合最高 DPS"),
        variables: &[
            variable!(
                "Value",
                "数值",
                "round_max_dps",
                "Best one-second DPS in the completed round.",
                "刚结束这回合中，表现最好的一秒 DPS。"
            ),
            variable!(
                "Condition",
                "条件",
                "has_round_max_dps",
                "Enabled when maximum DPS is available for this round.",
                "本回合有最高 DPS 可显示时开启。"
            ),
        ],
    },
    VariableCopyGroup {
        title: p("Round effective DPS", "回合有效 DPS"),
        variables: &[
            variable!(
                "Value",
                "数值",
                "round_report_effective_dps",
                "DPS while actively dealing damage; walking and waiting do not reduce it.",
                "只计算你持续输出时的 DPS，走路和等待不会拉低它。"
            ),
            variable!(
                "Condition",
                "条件",
                "has_round_report_effective_dps",
                "Enabled when effective DPS is available for this round.",
                "本回合有有效 DPS 可显示时开启。"
            ),
        ],
    },
    VariableCopyGroup {
        title: p("Round 10-second burst", "回合 10 秒爆发"),
        variables: &[
            variable!(
                "Value",
                "数值",
                "round_report_burst_10s",
                "Best rolling 10-second average DPS in the completed round.",
                "刚结束这回合表现最好的连续 10 秒平均 DPS。"
            ),
            variable!(
                "Condition",
                "条件",
                "has_round_report_burst_10s",
                "Enabled when the round lasted long enough to produce burst DPS.",
                "本回合打满过 10 秒、有爆发 DPS 可显示时开启。"
            ),
        ],
    },
    VariableCopyGroup {
        title: p("Effective DPS growth", "有效 DPS 增长率"),
        variables: &[
            variable!(
                "Value",
                "数值",
                "dps_growth_rate",
                "Change in effective DPS from the previous round; number only, 0 when unavailable.",
                "本回合有效 DPS 相比上一回合提升或下降多少。实际伤害已体现怪物抗性；只显示数字，不带百分号，无法比较时为 0。"
            ),
            variable!(
                "Condition",
                "条件",
                "has_dps_growth_rate",
                "Enabled when previous-round effective DPS is available for comparison.",
                "有上一回合的有效 DPS 可比较时开启。"
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
                "Longest period without pressing W, A, S, or D in the completed round, as a number of seconds without a unit.",
                "刚结束这回合中，最长有多久没按 W、A、S、D；只显示秒数，不带单位。"
            ),
            variable!(
                "Condition",
                "条件",
                "has_round_longest_standstill",
                "Enabled when movement was fully recorded and standstill data is available.",
                "软件完整记录了移动、有站桩时间可显示时开启。"
            ),
        ],
    },
    VariableCopyGroup {
        title: p("Round summary", "回合汇总"),
        variables: &[
            variable!(
                "Condition",
                "条件",
                "has_round_report",
                "Enabled immediately after a round ends and a report can be displayed.",
                "刚刚有一回合结束、可以显示战报时开启。"
            ),
            variable!(
                "Value",
                "数值",
                "round_total_damage",
                "Total damage dealt in the completed round.",
                "刚结束这回合一共打了多少伤害。"
            ),
            variable!(
                "Value",
                "数值",
                "round_report_damage_taken",
                "Total damage taken in the completed round.",
                "刚结束这回合一共受到多少伤害。"
            ),
        ],
    },
];

pub const BASIC_SYNTAX_HELP: &[SyntaxCopy] = &[
    SyntaxCopy {
        title: p("1 · Display a variable", "1 · 显示一个变量"),
        description: p(
            "Place a copied variable where you want its value. Ordinary text is preserved.",
            "把复制出的变量放到想显示的位置。普通文字会原样保留。",
        ),
        code: p("Current DPS: {{latest_dps}}", "当前 DPS: {{latest_dps}}"),
    },
    SyntaxCopy {
        title: p(
            "2 · Display only when data is available",
            "2 · 数据可用时才显示",
        ),
        description: p(
            "The inner text appears only when the condition is true. Opening and closing tags must be paired.",
            "条件成立才显示中间的文字。开头和结尾要成对。",
        ),
        code: p(
            "{{#if has_latest_dps}}\nCurrent DPS: {{latest_dps}}\n{{/if}}",
            "{{#if has_latest_dps}}\n当前 DPS: {{latest_dps}}\n{{/if}}",
        ),
    },
    SyntaxCopy {
        title: p("3 · Choose with if / else", "3 · if / else 二选一"),
        description: p(
            "Show the if section when true, otherwise show the else section.",
            "条件成立显示 if 部分，否则显示 else 部分。",
        ),
        code: p(
            "{{#if is_self_boss_locked}}\n⚠ Boss is targeting me\n{{else}}\nBoss is not targeting me\n{{/if}}",
            "{{#if is_self_boss_locked}}\n⚠ Boss 正在锁定我\n{{else}}\nBoss 当前没有锁定我\n{{/if}}",
        ),
    },
    SyntaxCopy {
        title: p("4 · Display when text is present", "4 · 有内容时才显示"),
        description: p(
            "boss and boss_lock are truthy when non-empty, so no separate condition variable is needed.",
            "boss、boss_lock 不为空时就会显示，不需要另找判断变量。",
        ),
        code: p(
            "{{#if boss}}Boss: {{boss}}{{/if}}\n{{#if boss_lock}}Target: {{boss_lock}}{{/if}}",
            "{{#if boss}}Boss: {{boss}}{{/if}}\n{{#if boss_lock}}目标: {{boss_lock}}{{/if}}",
        ),
    },
    SyntaxCopy {
        title: p("5 · Check nested conditions", "5 · 同时检查两层条件"),
        description: p(
            "First check for a report, then for a reliable round estimate. Close the most recently opened if first.",
            "先确认有战报，再确认回合估计可靠。后打开的 if 要先结束。",
        ),
        code: p(
            "{{#if has_round_report}}\nRound DPS: {{round_report_effective_dps}}\n{{#if has_step_estimate}}About {{until_boss_step}} rounds until Jim{{/if}}\n{{/if}}",
            "{{#if has_round_report}}\n回合 DPS: {{round_report_effective_dps}}\n{{#if has_step_estimate}}预计还剩 {{until_boss_step}} 回合到 Jim{{/if}}\n{{/if}}",
        ),
    },
    SyntaxCopy {
        title: p("6 · Add multiple alerts", "6 · 放入多个提醒"),
        description: p(
            "Each alert has its own condition; inactive alerts are hidden.",
            "每个提醒各自判断，没触发的提醒不会显示。",
        ),
        code: p(
            "{{#if rapid_damage_danger}}⚠ Rapid damage{{/if}}\n{{#if no_dps_for_10s}}⚠ No damage for 10 seconds{{/if}}\n{{#if no_wasd_for_10s}}⚠ No movement for 10 seconds{{/if}}",
            "{{#if rapid_damage_danger}}⚠ 快速掉血{{/if}}\n{{#if no_dps_for_10s}}⚠ 10 秒无输出{{/if}}\n{{#if no_wasd_for_10s}}⚠ 10 秒未移动{{/if}}",
        ),
    },
];

pub const ADVANCED_SYNTAX_HELP: &[SyntaxCopy] = &[
    SyntaxCopy {
        title: p("unless · Display when false", "unless · 条件不成立时显示"),
        description: p(
            "The opposite of if; useful for text about something not happening.",
            "它和 if 相反，适合写“没有发生某事时”的内容。",
        ),
        code: p(
            "{{#unless boss_lock}}Boss has no target{{/unless}}",
            "{{#unless boss_lock}}Boss 暂时没有锁定目标{{/unless}}",
        ),
    },
    SyntaxCopy {
        title: p("eq / ne · Equal / not equal", "eq / ne · 等于 / 不等于"),
        description: p(
            "Put the comparison inside if parentheses. Text values use English double quotes.",
            "把判断放进 if 的括号里。文字要放在英文双引号中。",
        ),
        code: p(
            "{{#if (eq phase \"COMBAT\")}}In combat{{/if}}\n{{#if (ne status \"LIVE\")}}Data is not ready{{/if}}",
            "{{#if (eq phase \"COMBAT\")}}战斗中{{/if}}\n{{#if (ne status \"LIVE\")}}数据还没准备好{{/if}}",
        ),
    },
    SyntaxCopy {
        title: p(
            "gt / gte / lt / lte · Numeric comparison",
            "gt / gte / lt / lte · 数字大小",
        ),
        description: p(
            "Greater than, greater than or equal, less than, and less than or equal.",
            "依次表示大于、大于等于、小于、小于等于。",
        ),
        code: p(
            "{{#if (gt round_damage_taken 50)}}Round damage taken exceeds 50{{/if}}",
            "{{#if (gt round_damage_taken 50)}}本回合承伤超过 50{{/if}}",
        ),
    },
    SyntaxCopy {
        title: p(
            "and / or / not · Combine conditions",
            "and / or / not · 组合判断",
        ),
        description: p(
            "and requires all conditions, or requires one, and not reverses the result.",
            "and 要全部成立，or 只要一个成立，not 会把结果反过来。",
        ),
        code: p(
            "{{#if (and has_step_estimate (eq until_boss_step \"0\"))}}Jim is expected next{{/if}}\n{{#if (or rapid_damage_danger no_dps_for_10s)}}⚠ Check combat status{{/if}}",
            "{{#if (and has_step_estimate (eq until_boss_step \"0\"))}}下一战预计是 Jim{{/if}}\n{{#if (or rapid_damage_danger no_dps_for_10s)}}⚠ 注意战斗状态{{/if}}",
        ),
    },
    SyntaxCopy {
        title: p("Comment · Notes for yourself", "注释 · 写给自己看"),
        description: p(
            "Comments are not included in the actual message.",
            "注释不会出现在实际消息里。",
        ),
        code: p(
            "{{! Show only when DPS is available }}\n{{#if has_latest_dps}}DPS: {{latest_dps}}{{/if}}",
            "{{! 下面只在有 DPS 时显示 }}\n{{#if has_latest_dps}}DPS: {{latest_dps}}{{/if}}",
        ),
    },
    SyntaxCopy {
        title: p("~ · Trim extra whitespace", "~ · 去掉多余空白"),
        description: p(
            "Add ~ inside braces to consume adjacent spaces or line breaks.",
            "在花括号内侧加 ~，可以吃掉标签旁边的空格或换行。",
        ),
        code: p(
            "DPS: {{latest_dps}}{{~#if boss}} | Boss: {{boss}}{{/if}}",
            "DPS: {{latest_dps}}{{~#if boss}} | Boss: {{boss}}{{/if}}",
        ),
    },
];

pub const TEMPLATE_HELP_TIPS: &[HelpTipCopy] = &[
    HelpTipCopy {
        question: p("Why does my template report an error?", "为什么模板报错？"),
        answer: p(
            "Usually a variable is misspelled, {{/if}} is missing, or nested if blocks are closed in the wrong order. Live Preview shows the error.",
            "通常是变量名拼错、漏写 {{/if}}，或两层 if 的结束顺序写反了。下方“实时预览”会显示错误。",
        ),
    },
    HelpTipCopy {
        question: p("Can I compare or calculate values?", "能写比较和计算吗？"),
        answer: p(
            "eq, ne, gt, gte, lt, and lte can compare values, but arithmetic is not currently supported.",
            "可以用 eq、ne、gt、gte、lt、lte 比较，但目前没有加减乘除。",
        ),
    },
    HelpTipCopy {
        question: p(
            "Why is the end of my message missing?",
            "为什么消息末尾不见了？",
        ),
        answer: p(
            "Before sending, the message is trimmed to VRChat Chatbox limits: 144 characters and 9 lines. Check the final text in Live Preview.",
            "发送前会按 VRChat Chatbox 限制裁剪为最多 144 个字符、9 行。换行和普通文字也计入长度，请在下方“实时预览”检查最终文本。",
        ),
    },
    HelpTipCopy {
        question: p("Will zero be hidden?", "0 会不会被隐藏？"),
        answer: p(
            "Do not use numeric variables directly as conditions. Use the corresponding has_xxx condition so a real 0 is still displayed.",
            "不要直接用数值变量作条件。使用对应的 has_xxx 判断数据是否可用，这样真实数值 0 仍能正常显示。",
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
        let pair = TextPair::new("Overview", "运行总览");
        assert_eq!(pair.get(Language::English), "Overview");
        assert_eq!(pair.get(Language::Chinese), "运行总览");
    }

    #[test]
    fn central_catalog_contains_no_empty_translation_halves() {
        let source = include_str!("i18n.rs");
        assert!(!source.contains("p(\"\","));
        assert!(!source.contains(", \"\")"));
        assert!(!source.contains("pair!(\"\""));
    }
}
