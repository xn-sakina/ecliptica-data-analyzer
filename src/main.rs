#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::{Arc, atomic::Ordering},
    time::{Duration, Instant},
};

#[cfg(not(target_os = "windows"))]
use std::process::Command;

use anyhow::Context;
use arboard::Clipboard;
use ecliptica_data_analyzer::{
    APP_ID, APP_NAME,
    analysis::{
        DataStatus, DpsHistoryPoint, GameSnapshot, RoundPhase, RoundReport, normalized_name,
    },
    audio::SoundCommand,
    config::{self, AlertSoundStyle, AppConfig, SendInterval},
    i18n::{Language, format_pattern, format_seconds_pattern, text},
    runtime::{AwayReason, EventLevel, EventPresentation, Runtime, SystemEvent, ToastLevel},
};
use eframe::egui;
use egui_plot::{Bar, BarChart, HLine, Line, MarkerShape, Plot, PlotPoints, Points, VLine};
use egui_shadcn::{
    Alert, AlertDialog, AlertDialogResult, AlertVariant, Badge, BadgeVariant,
    Button as ShadcnButton, ButtonVariant, ComponentSize, Dialog, Empty, Flex, Input, Item,
    LucideIcon, NumberInput, PropertyRow, ScrollArea as ShadcnScrollArea, SelectValue,
    ShadcnThemeExt, Slider as ShadcnSlider, Switch, Textarea, ToastState, ToastVariant,
    ToggleGroup, ToggleVariant, Typography, TypographyVariant,
};
use parking_lot::Mutex;
use single_instance::SingleInstance;
use tracing_subscriber::EnvFilter;

const OVERLAY_ID: &str = "ecliptica-overlay";
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const OVERLAY_WIDTH: f32 = 340.0;
const OVERLAY_COMPACT_HEIGHT: f32 = 204.0;
const OVERLAY_EXPANDED_HEIGHT: f32 = 252.0;
const OVERLAY_REPORT_WITH_ALERT_HEIGHT: f32 = 300.0;
const OVERLAY_CONTENT_WIDTH: f32 = 296.0;
const OVERLAY_ITEM_SPACING: egui::Vec2 = egui::vec2(6.0, 4.0);
const OVERLAY_SCALE_OPTIONS: [f32; 6] = [0.75, 1.0, 1.25, 1.5, 1.75, 2.0];
const OVERLAY_STATUS_DOT_COLOR: egui::Color32 = egui::Color32::from_rgb(83, 211, 225);
const UI_SPACE_1: f32 = 4.0;
const UI_SPACE_2: f32 = 8.0;
const UI_SPACE_3: f32 = 12.0;
const UI_SPACE_4: f32 = 16.0;
const UI_SPACE_5: f32 = 20.0;
const CJK_FONT_FAMILY: &str = "system-cjk";
const EXTENDED_TEXT_FONT_FAMILY: &str = "system-extended-text";
const SYMBOL_FONT_FAMILY: &str = "system-symbols";
const MAX_LOG_ROWS: usize = 200;
const DEVELOPER_MODE_CLICK_COUNT: u8 = 5;
const DEVELOPER_MODE_CLICK_TIMEOUT: Duration = Duration::from_secs(4);
const SETTINGS_BG: egui::Color32 = egui::Color32::from_rgb(15, 13, 20);
const SETTINGS_SIDEBAR_BG: egui::Color32 = egui::Color32::from_rgb(20, 17, 27);
const SETTINGS_SURFACE: egui::Color32 = egui::Color32::from_rgb(25, 22, 33);
const SETTINGS_SURFACE_HOVER: egui::Color32 = egui::Color32::from_rgb(35, 30, 46);
const SETTINGS_BORDER: egui::Color32 = egui::Color32::from_rgb(53, 47, 67);
const SETTINGS_ACCENT: egui::Color32 = egui::Color32::from_rgb(190, 174, 255);
const SETTINGS_PREVIEW_BG: egui::Color32 = egui::Color32::from_rgb(33, 28, 45);
const SETTINGS_PREVIEW_BORDER: egui::Color32 = egui::Color32::from_rgb(94, 81, 129);
const SETTINGS_INSET_BORDER: egui::Color32 = egui::Color32::from_rgba_premultiplied(34, 31, 46, 46);
const SETTINGS_CHART_BG: egui::Color32 = egui::Color32::from_rgb(29, 23, 43);
const SETTINGS_CHART_AXIS: egui::Color32 = egui::Color32::from_rgb(229, 222, 248);
const SETTINGS_CHART_CURSOR: egui::Color32 = egui::Color32::from_rgb(221, 207, 255);
const SETTINGS_CHART_LINE: egui::Color32 = egui::Color32::from_rgb(207, 190, 255);
const DPS_CHART_AUTO_FIT_IDLE: Duration = Duration::from_secs(5);
const DPS_CHART_AUTO_FIT_INTERVAL: Duration = Duration::from_secs(5);
const DPS_CHART_RECENT_WINDOW_SECONDS: f64 = 5.0 * 60.0;
const DPS_CHART_MAX_TREND_POINTS: usize = 600;
const DPS_CHART_PEAK_HIT_RADIUS: f32 = 12.0;
const DPS_CHART_X_AXIS_TITLE_GAP: f32 = 4.0;
const DPS_CHART_X_MARGIN_FRACTION: f64 = 0.025;
const DPS_CHART_Y_MARGIN_FRACTION: f64 = 0.10;
const DPS_CHART_MIN_Y_SPAN: f64 = 10.0;
const TEMPLATE_PRESET_TAB_ROW_HEIGHT: f32 = 28.0;
const TEMPLATE_PRESET_TAB_LABEL_MAX_CHARS: usize = 13;
const ALERT_SOUND_LABEL_WIDTH_ENGLISH: f32 = 200.0;
const ALERT_SOUND_LABEL_WIDTH_CHINESE: f32 = 124.0;
const HEART_RATE_GUIDE_URL: &str =
    "https://github.com/xn-sakina/ecliptica-data-analyzer/blob/main/resources/heart-rate/README.md";
const SETTINGS_TEXT: egui::Color32 = egui::Color32::from_rgb(246, 243, 255);
const SETTINGS_HEADING: egui::Color32 = egui::Color32::from_rgb(225, 218, 255);
const SETTINGS_TEXT_SECONDARY: egui::Color32 = egui::Color32::from_rgb(190, 183, 204);
const SETTINGS_TEXT_MUTED: egui::Color32 = egui::Color32::from_rgb(148, 141, 164);
const SETTINGS_SUCCESS: egui::Color32 = egui::Color32::from_rgb(105, 221, 153);
const SETTINGS_INFO: egui::Color32 = egui::Color32::from_rgb(119, 181, 255);
const SETTINGS_WARNING: egui::Color32 = egui::Color32::from_rgb(255, 204, 102);
const SETTINGS_DANGER: egui::Color32 = egui::Color32::from_rgb(255, 132, 146);
// Shared metric colors keep the dashboard, Overlay, and reports consistent.
// The chart and template-variable categories intentionally have their own
// palettes because they need different kinds of visual separation.
const METRIC_LIVE_DPS: egui::Color32 = SETTINGS_INFO;
const METRIC_AVERAGE_DPS: egui::Color32 = SETTINGS_ACCENT;
const METRIC_ACTIVE_DPS: egui::Color32 = SETTINGS_SUCCESS;
const METRIC_BEST_DPS: egui::Color32 = SETTINGS_WARNING;
const METRIC_DAMAGE_TAKEN: egui::Color32 = SETTINGS_DANGER;
const METRIC_BOSS_LOCK: egui::Color32 = egui::Color32::from_rgb(83, 211, 225);
const METRIC_DURATION: egui::Color32 = SETTINGS_INFO;
const METRIC_TOTAL_DAMAGE: egui::Color32 = SETTINGS_ACCENT;
const METRIC_DPS_GROWTH: egui::Color32 = SETTINGS_SUCCESS;
const METRIC_STANDSTILL: egui::Color32 = SETTINGS_WARNING;
const METRIC_HEART_RATE: egui::Color32 = SETTINGS_DANGER;
const VARIABLE_HIGHEST_DPS: egui::Color32 = egui::Color32::from_rgb(143, 156, 255);
const VARIABLE_BEST_AVERAGE_DPS: egui::Color32 = egui::Color32::from_rgb(218, 148, 255);
const VARIABLE_COMBAT: egui::Color32 = METRIC_BOSS_LOCK;
const VARIABLE_ALERT: egui::Color32 = egui::Color32::from_rgb(240, 140, 203);
const VARIABLE_GAME_PROGRESS: egui::Color32 = egui::Color32::from_rgb(96, 210, 190);
const VARIABLE_DURATION: egui::Color32 = egui::Color32::from_rgb(214, 207, 225);
const VARIABLE_DPS_GROWTH: egui::Color32 = egui::Color32::from_rgb(141, 221, 248);
const VARIABLE_STANDSTILL: egui::Color32 = egui::Color32::from_rgb(255, 153, 184);
const VARIABLE_ROUND_DAMAGE: egui::Color32 = egui::Color32::from_rgb(232, 133, 202);
const VARIABLE_HEART_RATE: egui::Color32 = METRIC_HEART_RATE;

fn alert_sound_label_width(language: Language) -> f32 {
    match language {
        Language::English => ALERT_SOUND_LABEL_WIDTH_ENGLISH,
        Language::Chinese => ALERT_SOUND_LABEL_WIDTH_CHINESE,
    }
}
const SIDEBAR_FOOTER_BASE_HEIGHT: f32 = 116.0;
const SIDEBAR_ERROR_DETAILS_HEIGHT: f32 = 34.0;
const SETTINGS_STATUS_SLOT_WIDTH: f32 = 104.0;

// Keep secondary copy comfortably readable on both opaque settings panels and
// the translucent overlay. Avoid egui's default 60% weak-text treatment.
const TEXT_SECONDARY: egui::Color32 = egui::Color32::from_gray(196);

fn main() -> anyhow::Result<()> {
    let startup_language = Language::system_default();
    let instance =
        SingleInstance::new(APP_ID).context(text::SINGLE_INSTANCE_FAILED.get(startup_language))?;
    if !instance.is_single() {
        anyhow::bail!(
            "{} v{APP_VERSION}",
            text::ALREADY_RUNNING.get(startup_language)
        );
    }

    let log_dir = config::config_dir()?.join("logs");
    std::fs::create_dir_all(&log_dir)?;
    let file_appender = tracing_appender::rolling::daily(log_dir, "ecliptica.log");
    let (writer, _logging_guard) = tracing_appender::non_blocking(file_appender);
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(writer)
        .with_ansi(false)
        .init();

    let (loaded_config, recovery) = config::load_or_recover()?;
    let runtime = Runtime::start(loaded_config.clone());
    if let Some(message) = recovery {
        runtime.shared.event(EventLevel::Warning, message);
    }

    let shutdown = runtime.shared.shutdown.clone();
    ctrlc::set_handler(move || shutdown.store(true, Ordering::SeqCst))
        .context(text::EXIT_HANDLER_FAILED.get(loaded_config.language))?;

    let window_title = format!("{APP_NAME} v{APP_VERSION}");
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_app_id(APP_ID)
            .with_title(window_title.clone())
            .with_icon(app_icon())
            // Glow chooses its shared framebuffer format from the root viewport.
            // Request alpha here so transparent deferred viewports work on macOS;
            // the settings window still paints an opaque SETTINGS_BG panel.
            .with_transparent(true)
            .with_inner_size([940.0, 720.0])
            .with_min_inner_size([760.0, 600.0]),
        centered: true,
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        &window_title,
        options,
        Box::new(move |creation| {
            install_cjk_font(&creation.egui_ctx);
            install_theme(&creation.egui_ctx);
            Ok(Box::new(AnalyzerApp::new(
                &creation.egui_ctx,
                runtime,
                loaded_config,
            )))
        }),
    )
    .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    drop(instance);
    Ok(())
}

fn app_icon() -> egui::IconData {
    const ICON_SIDE: u32 = 256;
    let rgba = include_bytes!("../assets/app-icon-256.rgba").to_vec();
    debug_assert_eq!(
        rgba.len(),
        (ICON_SIDE * ICON_SIDE * 4) as usize,
        "embedded application icon must be 256x256 RGBA"
    );
    egui::IconData {
        rgba,
        width: ICON_SIDE,
        height: ICON_SIDE,
    }
}

#[derive(Clone)]
struct LogRow {
    time: String,
    level: EventLevel,
    message: String,
    repeats: u32,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum SettingsPage {
    Overview,
    Message,
    Player,
    Overlay,
    Logs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelfLockTransition {
    Locked,
    Unlocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TemplatePreviewState {
    Normal,
    RoundReport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SidebarNoticeTone {
    Success,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SendIntervalChoice(SendInterval, Language);

impl std::fmt::Display for SendIntervalChoice {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&format_seconds_pattern(self.1, self.0.seconds_label()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AlertSoundStyleChoice(AlertSoundStyle, Language);

impl std::fmt::Display for AlertSoundStyleChoice {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.0.display_label(self.1))
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct OverlayScaleChoice(f32);

impl std::fmt::Display for OverlayScaleChoice {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0.fract() == 0.0 {
            write!(formatter, "{:.0}×", self.0)
        } else {
            write!(formatter, "{}×", self.0)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AwayDuration {
    One,
    Three,
    Five,
    Ten,
}

impl AwayDuration {
    const ALL: [Self; 4] = [Self::One, Self::Three, Self::Five, Self::Ten];

    fn duration(self) -> Duration {
        Duration::from_secs(match self {
            Self::One => 60,
            Self::Three => 180,
            Self::Five => 300,
            Self::Ten => 600,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AwayReasonChoice(AwayReason, Language);

impl std::fmt::Display for AwayReasonChoice {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self.0 {
            AwayReason::Takeout => text::AWAY_REASON_TAKEOUT.get(self.1),
            AwayReason::Restroom => text::AWAY_REASON_RESTROOM.get(self.1),
            AwayReason::Custom => text::AWAY_REASON_CUSTOM.get(self.1),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AwayDurationChoice(AwayDuration, Language);

impl std::fmt::Display for AwayDurationChoice {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self.0 {
            AwayDuration::One => text::AWAY_ONE_MINUTE.get(self.1),
            AwayDuration::Three => text::AWAY_THREE_MINUTES.get(self.1),
            AwayDuration::Five => text::AWAY_FIVE_MINUTES.get(self.1),
            AwayDuration::Ten => text::AWAY_TEN_MINUTES.get(self.1),
        })
    }
}

struct AwayDialogState {
    open: bool,
    exit_confirm_open: bool,
    reason: AwayReason,
    duration: AwayDuration,
    custom_message: String,
}

struct AnalyzerApp {
    runtime: Runtime,
    persisted: AppConfig,
    draft: AppConfig,
    logs: VecDeque<LogRow>,
    developer_logs: VecDeque<LogRow>,
    alert: Option<(String, Instant, EventLevel)>,
    save_result: Option<(String, bool)>,
    save_error_detail: Option<String>,
    save_error_detail_open: bool,
    previous_snapshot: GameSnapshot,
    last_lock_sound: Option<Instant>,
    last_unlock_sound: Option<Instant>,
    template_preview_state: TemplatePreviewState,
    dps_chart_view: DpsChartViewState,
    clipboard: Option<Clipboard>,
    toast_state: ToastState,
    template_help_open: bool,
    page: SettingsPage,
    reset_confirm_open: bool,
    template_preset_reset_confirm: Option<TemplatePresetResetKind>,
    window_always_on_top: bool,
    developer_mode: bool,
    developer_logs_open: bool,
    away_dialog: AwayDialogState,
    developer_logo_clicks: u8,
    last_developer_logo_click: Option<Instant>,
    overlay_position: Arc<Mutex<OverlayPositionState>>,
}

#[derive(Default)]
struct OverlayPositionState {
    pending: Option<egui::Pos2>,
    last_observed: Option<egui::Pos2>,
}

#[derive(Debug, Clone, Copy, Default)]
struct DpsChartViewState {
    selected_epoch: Option<u64>,
    last_user_interaction: Option<Instant>,
    last_auto_fit: Option<Instant>,
    interaction_rect: Option<egui::Rect>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TemplatePresetResetKind {
    Message,
    Report,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ChartRoundMarker {
    start_seconds: f64,
    step: u32,
}

impl DpsChartViewState {
    fn record_user_interaction(&mut self, now: Instant) {
        self.last_user_interaction = Some(now);
    }

    fn should_auto_fit(&self, now: Instant) -> bool {
        if let Some(last_user_interaction) = self.last_user_interaction {
            if now.saturating_duration_since(last_user_interaction) < DPS_CHART_AUTO_FIT_IDLE {
                return false;
            }
            if self
                .last_auto_fit
                .is_none_or(|last_auto_fit| last_auto_fit < last_user_interaction)
            {
                return true;
            }
        }
        self.last_auto_fit.is_none_or(|last_auto_fit| {
            now.saturating_duration_since(last_auto_fit) >= DPS_CHART_AUTO_FIT_INTERVAL
        })
    }

    fn record_auto_fit(&mut self, now: Instant) {
        self.last_auto_fit = Some(now);
    }

    fn pointer_is_in_chart(&self, ui: &egui::Ui) -> bool {
        self.interaction_rect.is_some_and(|rect| {
            ui.input(|input| {
                input
                    .pointer
                    .hover_pos()
                    .is_some_and(|position| rect.contains(position))
            })
        })
    }

    fn next_auto_fit_in(&self, now: Instant) -> Duration {
        if let Some(last_user_interaction) = self.last_user_interaction {
            if self
                .last_auto_fit
                .is_none_or(|last_auto_fit| last_auto_fit < last_user_interaction)
            {
                return DPS_CHART_AUTO_FIT_IDLE
                    .saturating_sub(now.saturating_duration_since(last_user_interaction));
            }
        }
        self.last_auto_fit.map_or(Duration::ZERO, |last_auto_fit| {
            DPS_CHART_AUTO_FIT_INTERVAL.saturating_sub(now.saturating_duration_since(last_auto_fit))
        })
    }
}

impl AnalyzerApp {
    fn new(_ctx: &egui::Context, runtime: Runtime, draft: AppConfig) -> Self {
        let away_custom_message = draft.away_custom_message.clone();
        Self {
            runtime,
            persisted: draft.clone(),
            draft,
            logs: VecDeque::new(),
            developer_logs: VecDeque::new(),
            alert: None,
            save_result: None,
            save_error_detail: None,
            save_error_detail_open: false,
            previous_snapshot: GameSnapshot::default(),
            last_lock_sound: None,
            last_unlock_sound: None,
            template_preview_state: TemplatePreviewState::Normal,
            dps_chart_view: DpsChartViewState::default(),
            clipboard: None,
            toast_state: ToastState::new(),
            template_help_open: false,
            page: SettingsPage::Overview,
            reset_confirm_open: false,
            template_preset_reset_confirm: None,
            window_always_on_top: false,
            developer_mode: false,
            developer_logs_open: false,
            away_dialog: AwayDialogState {
                open: false,
                exit_confirm_open: false,
                reason: AwayReason::Restroom,
                duration: AwayDuration::Three,
                custom_message: away_custom_message,
            },
            developer_logo_clicks: 0,
            last_developer_logo_click: None,
            overlay_position: Arc::new(Mutex::new(OverlayPositionState::default())),
        }
    }

    fn sync_overlay_position(&mut self) {
        let Some(position) = self.overlay_position.lock().pending.take() else {
            return;
        };
        if (self.draft.overlay_x - position.x).abs() >= 0.5
            || (self.draft.overlay_y - position.y).abs() >= 0.5
        {
            self.draft.overlay_x = position.x;
            self.draft.overlay_y = position.y;
            let mut live = self.runtime.shared.config.write();
            live.value.overlay_x = position.x;
            live.value.overlay_y = position.y;
        }
    }

    fn process_events(&mut self, ctx: &egui::Context) {
        let toast_time = ctx.input(|input| input.time);
        while let Ok(event) = self.runtime.events.try_recv() {
            if let EventPresentation::Toast(level) = event.presentation {
                self.toast_state.add(
                    event.message.clone(),
                    toast_variant_for_event(level),
                    toast_time,
                );
            }
            self.push_log(event);
        }
        if self
            .alert
            .as_ref()
            .is_some_and(|(_, deadline, _)| Instant::now() >= *deadline)
        {
            self.alert = None;
        }
    }

    fn push_log(&mut self, event: SystemEvent) {
        if is_protocol_diagnostic(&event.message) {
            push_log_row(&mut self.developer_logs, event);
            return;
        }
        if let Some(last) = self.logs.back_mut() {
            if last.message == event.message && last.level == event.level {
                last.repeats = last.repeats.saturating_add(1);
                return;
            }
        }
        if event_creates_overlay_alert(&event) {
            self.alert = Some((
                event.message.clone(),
                Instant::now() + Duration::from_secs(6),
                event.level,
            ));
        }
        self.logs.push_back(LogRow {
            time: chrono::Local::now().format("%H:%M:%S").to_string(),
            level: event.level,
            message: event.message,
            repeats: 1,
        });
        while self.logs.len() > MAX_LOG_ROWS {
            self.logs.pop_front();
        }
    }

    fn register_developer_logo_click(&mut self, ctx: &egui::Context) {
        let now = Instant::now();
        if register_hidden_click(
            &mut self.developer_logo_clicks,
            &mut self.last_developer_logo_click,
            now,
        ) {
            self.developer_mode = !self.developer_mode;
            if !self.developer_mode {
                self.developer_logs_open = false;
            }
            self.toast_state.add(
                if self.developer_mode {
                    text::DEVELOPER_MODE_ENABLED.get(self.draft.language)
                } else {
                    text::DEVELOPER_MODE_DISABLED.get(self.draft.language)
                },
                ToastVariant::Success,
                ctx.input(|input| input.time),
            );
        }
    }

    fn detect_self_lock_edge(&mut self, snapshot: &GameSnapshot) {
        let display_name = self.runtime.shared.config.read().value.display_name.clone();
        match self_lock_transition(&self.previous_snapshot, snapshot, &display_name) {
            Some(SelfLockTransition::Locked)
                if self
                    .last_lock_sound
                    .is_none_or(|last| last.elapsed() >= Duration::from_secs(5)) =>
            {
                let (volume, style) = {
                    let config = self.runtime.shared.config.read();
                    (config.value.alert_volume, config.value.locked_sound_style)
                };
                let _ = self
                    .runtime
                    .sounds
                    .try_send(SoundCommand::Locked(volume, style));
                self.last_lock_sound = Some(Instant::now());
            }
            Some(SelfLockTransition::Unlocked)
                if self
                    .last_unlock_sound
                    .is_none_or(|last| last.elapsed() >= Duration::from_secs(2)) =>
            {
                let (volume, style) = {
                    let config = self.runtime.shared.config.read();
                    (config.value.alert_volume, config.value.unlocked_sound_style)
                };
                let _ = self
                    .runtime
                    .sounds
                    .try_send(SoundCommand::Unlocked(volume, style));
                self.last_unlock_sound = Some(Instant::now());
            }
            _ => {}
        }
        self.previous_snapshot = snapshot.clone();
    }

    fn save(&mut self) {
        let language = self.draft.language;
        self.draft.version = config::CONFIG_VERSION;
        self.draft.sync_active_message_template_preset();
        self.draft.sync_active_round_report_template_preset();
        match config::save_atomic(&self.draft) {
            Ok(()) => {
                self.persisted = self.draft.clone();
                let mut live = self.runtime.shared.config.write();
                live.value = self.draft.clone();
                live.revision = live.revision.wrapping_add(1);
                drop(live);
                self.save_result = Some((text::SAVE_SUCCESS.get(language).to_owned(), true));
                self.save_error_detail = None;
                self.save_error_detail_open = false;
                self.push_log(SystemEvent {
                    level: EventLevel::Info,
                    message: text::CONFIG_SAVED_LOG.get(language).to_owned(),
                    presentation: EventPresentation::Default,
                });
            }
            Err(error) => {
                let detail = format!("{}: {error:#}", text::SAVE_FAILED.get(language));
                self.save_result = Some((detail.clone(), false));
                self.save_error_detail = Some(detail);
                self.save_error_detail_open = true;
            }
        }
    }

    /// Persist the language independently from the settings draft.
    ///
    /// The live configuration is the last committed configuration, so cloning
    /// it here guarantees that unrelated edits still held in `self.draft`
    /// never leak into the language-only save.
    fn save_language_immediately(&mut self, language: Language) {
        let persisted = config_with_language(&self.persisted, language);
        let draft = config_with_language(&self.draft, language);

        match config::save_atomic(&persisted) {
            Ok(()) => {
                let mut live = self.runtime.shared.config.write();
                let templates_changed = apply_language_managed_fields(&mut live.value, &persisted);
                if templates_changed {
                    live.revision = live.revision.wrapping_add(1);
                }
                drop(live);

                // Keep unrelated draft fields untouched. Only untouched built-in
                // templates and names follow the selected language.
                self.persisted = persisted;
                self.draft = draft;
                config::localize_away_custom_message(
                    &mut self.away_dialog.custom_message,
                    language,
                );
                self.save_result = Some((text::LANGUAGE_SAVED.get(language).to_owned(), true));
                self.save_error_detail = None;
                self.save_error_detail_open = false;
                self.push_log(SystemEvent {
                    level: EventLevel::Info,
                    message: text::LANGUAGE_SAVED_LOG.get(language).to_owned(),
                    presentation: EventPresentation::Default,
                });
            }
            Err(error) => {
                let detail = format!("{}: {error:#}", text::SAVE_FAILED.get(language));
                self.save_result = Some((detail.clone(), false));
                self.save_error_detail = Some(detail);
                self.save_error_detail_open = true;
            }
        }
    }

    fn has_unsaved_changes(&self) -> bool {
        self.draft != self.persisted
    }

    fn settings_ui(&mut self, ctx: &egui::Context, snapshot: &GameSnapshot) {
        let has_unsaved_changes = self.has_unsaved_changes();
        let language = self.draft.language;
        // The root viewport needs alpha for the separate transparent Overlay.
        // Paint an opaque foundation behind every management panel so one-pixel
        // panel seams never reveal the desktop or another application.
        #[allow(deprecated)]
        let management_rect = ctx.screen_rect();
        ctx.layer_painter(egui::LayerId::background()).rect_filled(
            management_rect,
            egui::CornerRadius::ZERO,
            SETTINGS_BG,
        );
        egui::SidePanel::left("settings-nav")
            .exact_width(214.0)
            .resizable(false)
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(SETTINGS_SIDEBAR_BG)
                    .inner_margin(egui::Margin {
                        left: UI_SPACE_3 as i8,
                        right: UI_SPACE_3 as i8,
                        top: UI_SPACE_3 as i8,
                        bottom: 0,
                    })
                    .stroke(egui::Stroke::NONE),
            )
            .show(ctx, |ui| {
                let has_save_error = self
                    .save_result
                    .as_ref()
                    .is_some_and(|(_, succeeded)| !succeeded);
                let save_notice = match self.save_result.as_ref() {
                    Some((_, false)) => Some((
                        text::SAVE_FAILED_VIEW_ERROR.get(language).to_owned(),
                        SidebarNoticeTone::Error,
                    )),
                    _ if has_unsaved_changes => Some((
                        text::UNSAVED_CHANGES.get(language).to_owned(),
                        SidebarNoticeTone::Warning,
                    )),
                    Some((message, true)) => Some((message.clone(), SidebarNoticeTone::Success)),
                    None => None,
                };
                let sidebar_content_rect = ui.max_rect();
                ui.painter().vline(
                    sidebar_content_rect.right() + UI_SPACE_3,
                    management_rect.y_range(),
                    egui::Stroke::new(1.0, SETTINGS_BORDER),
                );
                let sidebar_rect = ui.max_rect();
                let notice_height = save_notice
                    .as_ref()
                    .map(|(message, _)| {
                        sidebar_notice_height(ui, message.as_str(), sidebar_rect.width())
                            + UI_SPACE_2
                    })
                    .unwrap_or_default()
                    + if has_save_error {
                        SIDEBAR_ERROR_DETAILS_HEIGHT
                    } else {
                        0.0
                    };
                let footer_height = SIDEBAR_FOOTER_BASE_HEIGHT + notice_height;
                let footer_top = sidebar_rect.bottom() - footer_height;
                let body_rect = egui::Rect::from_min_max(
                    sidebar_rect.min,
                    egui::pos2(sidebar_rect.max.x, footer_top),
                );
                let footer_rect = egui::Rect::from_min_max(
                    egui::pos2(sidebar_rect.min.x, footer_top),
                    sidebar_rect.max,
                );
                let mut footer_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .id_salt("settings-nav-footer")
                        .max_rect(footer_rect)
                        .layout(egui::Layout::bottom_up(egui::Align::LEFT)),
                );
                footer_ui.add_space(UI_SPACE_2);
                if ShadcnButton::new(text::RESTORE_DEFAULTS.get(language))
                    .icon(LucideIcon::RotateCcw)
                    .variant(ButtonVariant::Outline)
                    .full_width()
                    .height(40.0)
                    .horizontal_padding(UI_SPACE_3)
                    .corner_radius(6.0)
                    .show(&mut footer_ui)
                    .on_hover_text(text::RESTORE_DEFAULTS_HINT.get(language))
                    .clicked()
                {
                    self.reset_confirm_open = true;
                }
                footer_ui.add_space(UI_SPACE_2);
                let (save_label, save_icon, save_variant) = if has_unsaved_changes {
                    (
                        text::SAVE_AND_APPLY.get(language),
                        LucideIcon::SaveAll,
                        ButtonVariant::Default,
                    )
                } else {
                    (
                        text::SAVED.get(language),
                        LucideIcon::Check,
                        ButtonVariant::Outline,
                    )
                };
                if ShadcnButton::new(save_label)
                    .icon(save_icon)
                    .variant(save_variant)
                    .enabled(has_unsaved_changes)
                    .full_width()
                    .height(40.0)
                    .horizontal_padding(UI_SPACE_3)
                    .corner_radius(6.0)
                    .show(&mut footer_ui)
                    .on_hover_text(if has_unsaved_changes {
                        text::SAVE_AND_APPLY_HINT.get(language)
                    } else {
                        text::NO_CHANGES_TO_SAVE.get(language)
                    })
                    .clicked()
                {
                    self.save();
                }
                if let Some((message, tone)) = save_notice.as_ref() {
                    footer_ui.add_space(UI_SPACE_2);
                    if has_save_error {
                        if ShadcnButton::new(text::VIEW_FULL_ERROR.get(language))
                            .icon(LucideIcon::CircleAlert)
                            .variant(ButtonVariant::Ghost)
                            .size(ComponentSize::Xs)
                            .show(&mut footer_ui)
                            .clicked()
                        {
                            self.save_error_detail_open = true;
                        }
                        footer_ui.add_space(UI_SPACE_1);
                    }
                    sidebar_notice(&mut footer_ui, message, *tone);
                }

                let mut body_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .id_salt("settings-nav-body")
                        .max_rect(body_rect)
                        .layout(egui::Layout::top_down(egui::Align::LEFT)),
                );
                let logo = body_ui.vertical(|ui| {
                    Typography::new("ECLIPTICA")
                        .font_size(16.0)
                        .strong()
                        .show(ui);
                    ui.add_space(UI_SPACE_1);
                    ui.horizontal(|ui| {
                        Typography::small("DATA ANALYZER")
                            .color(SETTINGS_TEXT_SECONDARY)
                            .show(ui);
                        Badge::new(format!("v{APP_VERSION}")).show(ui);
                    });
                });
                if logo
                    .response
                    .interact(egui::Sense::click())
                    .on_hover_cursor(egui::CursorIcon::Default)
                    .clicked()
                {
                    self.register_developer_logo_click(ctx);
                }
                body_ui.add_space(UI_SPACE_5);
                Typography::small(text::WORKSPACE.get(language))
                    .color(SETTINGS_TEXT_SECONDARY)
                    .show(&mut body_ui);
                body_ui.add_space(UI_SPACE_2);
                nav_button(
                    &mut body_ui,
                    &mut self.page,
                    SettingsPage::Overview,
                    text::OVERVIEW.get(language),
                    LucideIcon::LayoutDashboard,
                );
                nav_button(
                    &mut body_ui,
                    &mut self.page,
                    SettingsPage::Message,
                    text::OSC_MESSAGES.get(language),
                    LucideIcon::MessageSquare,
                );
                nav_button(
                    &mut body_ui,
                    &mut self.page,
                    SettingsPage::Player,
                    text::PLAYER_ALERTS.get(language),
                    LucideIcon::User,
                );
                nav_button(
                    &mut body_ui,
                    &mut self.page,
                    SettingsPage::Overlay,
                    text::OVERLAY.get(language),
                    LucideIcon::Monitor,
                );
                nav_button(
                    &mut body_ui,
                    &mut self.page,
                    SettingsPage::Logs,
                    text::SYSTEM_LOGS.get(language),
                    LucideIcon::ScrollText,
                );
            });

        egui::TopBottomPanel::top("app-header")
            .exact_height(48.0)
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(SETTINGS_SURFACE)
                    .inner_margin(egui::Margin::symmetric(UI_SPACE_5 as i8, 0))
                    .stroke(egui::Stroke::NONE),
            )
            .show(ctx, |ui| {
                ui.painter().hline(
                    ui.max_rect().expand2(egui::vec2(UI_SPACE_5, 0.0)).x_range(),
                    ui.max_rect().bottom() - 1.0,
                    egui::Stroke::new(1.0, SETTINGS_BORDER),
                );
                ui.allocate_ui_with_layout(
                    ui.available_size(),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            settings_status_pill(ui, snapshot.status, language);
                            let world = if snapshot.in_ecliptica {
                                text::ECLIPTICA_DETECTED.get(language)
                            } else {
                                text::WAITING_FOR_ECLIPTICA.get(language)
                            };
                            Typography::new(world)
                                .color(if snapshot.in_ecliptica {
                                    SETTINGS_SUCCESS
                                } else {
                                    SETTINGS_TEXT_SECONDARY
                                })
                                .show(ui);
                            ui.add_space(UI_SPACE_2);
                            let mut selected_language = self.draft.language;
                            let language_response =
                                SelectValue::new(&mut selected_language, &Language::ALL)
                                    .width(104.0)
                                    .show(ui)
                                    .on_hover_text(text::LANGUAGE_TOOLTIP.get(language));
                            if language_response.changed()
                                || selected_language != self.draft.language
                            {
                                self.save_language_immediately(selected_language);
                            }
                        });
                    },
                );
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(SETTINGS_BG))
            .show(ctx, |ui| {
                ShadcnScrollArea::new(ui.available_height())
                    .id_salt(("settings-page-scroll", self.page))
                    .framed(false)
                    .fill_available(true)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        egui::Frame::NONE
                            .inner_margin(egui::Margin::same(UI_SPACE_5 as i8))
                            .show(ui, |ui| match self.page {
                                SettingsPage::Overview => self.overview_page(ui, snapshot),
                                SettingsPage::Message => self.message_page(ui, snapshot),
                                SettingsPage::Player => self.player_page(ui),
                                SettingsPage::Overlay => self.overlay_page(ui),
                                SettingsPage::Logs => self.logs_page(ui),
                            });
                    });
            });

        if matches!(
            AlertDialog::new(
                text::RESTORE_DEFAULTS_TITLE.get(language),
                text::RESTORE_DEFAULTS_DESCRIPTION.get(language),
            )
            .close_label(text::CLOSE_DIALOG.get(language))
            .cancel_text(text::CANCEL.get(language))
            .action_text(text::RESTORE.get(language))
            .destructive()
            .show(ctx, &mut self.reset_confirm_open),
            AlertDialogResult::Confirmed
        ) {
            self.draft = AppConfig::defaults_for_language(self.draft.language);
            self.save_result = Some((text::DEFAULTS_RESTORED.get(language).to_owned(), true));
        }

        if let Some(kind) = self.template_preset_reset_confirm {
            let (title, description, notice) = match kind {
                TemplatePresetResetKind::Message => {
                    let active = self.draft.active_message_template_preset;
                    let name = preset_display_name(
                        &self.draft.message_template_preset_names[active],
                        active,
                        language,
                    );
                    (
                        text::RESET_MESSAGE_PRESET_TITLE.get(language),
                        format_pattern(
                            text::RESET_MESSAGE_PRESET_DESCRIPTION,
                            language,
                            &[("name", name.clone())],
                        ),
                        format_pattern(text::MESSAGE_PRESET_RESET, language, &[("name", name)]),
                    )
                }
                TemplatePresetResetKind::Report => {
                    let active = self.draft.active_round_report_template_preset;
                    let name = preset_display_name(
                        &self.draft.round_report_template_preset_names[active],
                        active,
                        language,
                    );
                    (
                        text::RESET_REPORT_PRESET_TITLE.get(language),
                        format_pattern(
                            text::RESET_REPORT_PRESET_DESCRIPTION,
                            language,
                            &[("name", name.clone())],
                        ),
                        format_pattern(text::REPORT_PRESET_RESET, language, &[("name", name)]),
                    )
                }
            };
            let mut open = true;
            match AlertDialog::new(title, description)
                .close_label(text::CLOSE_DIALOG.get(language))
                .cancel_text(text::CANCEL.get(language))
                .action_text(text::RESTORE.get(language))
                .destructive()
                .show(ctx, &mut open)
            {
                AlertDialogResult::Confirmed => {
                    match kind {
                        TemplatePresetResetKind::Message => {
                            self.draft.reset_active_message_template_to_default();
                        }
                        TemplatePresetResetKind::Report => {
                            self.draft.reset_active_round_report_template_to_default();
                        }
                    }
                    self.save_result = Some((notice, true));
                    self.template_preset_reset_confirm = None;
                }
                AlertDialogResult::Cancelled => {
                    self.template_preset_reset_confirm = None;
                }
                AlertDialogResult::Open => {}
            }
        }

        #[allow(deprecated)]
        let template_help_height =
            (ctx.input(|input| input.screen_rect().height()) - 170.0).clamp(500.0, 640.0);
        Dialog::new()
            .title(text::TEMPLATE_SYNTAX_HELP.get(language))
            .close_label(text::CLOSE_DIALOG.get(language))
            .width(680.0)
            .show(ctx, &mut self.template_help_open, |ui| {
                ui.set_min_height(template_help_height);
                template_syntax_help(ui, language, template_help_height);
            });
        let save_error_detail = self.save_error_detail.clone().unwrap_or_default();
        Dialog::new()
            .title(text::SAVE_FAILED.get(language))
            .close_label(text::CLOSE_DIALOG.get(language))
            .width(720.0)
            .show(ctx, &mut self.save_error_detail_open, |ui| {
                save_error_detail_dialog(ui, &save_error_detail, language);
            });
        let developer_logs = &self.developer_logs;
        #[allow(deprecated)]
        let developer_log_height =
            (ctx.input(|input| input.screen_rect().height()) - 170.0).clamp(480.0, 640.0);
        Dialog::new()
            .title(text::DEVELOPER_LOGS.get(language))
            .description(text::DEVELOPER_LOGS_DESCRIPTION.get(language))
            .close_label(text::CLOSE_DIALOG.get(language))
            .width(720.0)
            .show(ctx, &mut self.developer_logs_open, |ui| {
                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), developer_log_height),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        ShadcnScrollArea::new(developer_log_height)
                            .id_salt("developer-log-dialog")
                            .framed(false)
                            .stick_to_bottom(true)
                            .auto_shrink([false, false])
                            .fill_available(true)
                            .show(ui, |ui| {
                                for row in developer_logs {
                                    log_line(ui, row, language);
                                }
                                if developer_logs.is_empty() {
                                    Empty::show(ui, |ui| {
                                        Typography::muted(
                                            text::NO_DEVELOPER_DIAGNOSTICS.get(language),
                                        )
                                        .show(ui);
                                    });
                                }
                            });
                    },
                );
            });
        self.show_away_dialog(ctx);
        self.toast_state.show(ctx);
    }

    fn overview_page(&mut self, ui: &mut egui::Ui, snapshot: &GameSnapshot) {
        let language = self.draft.language;
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                Typography::h3(text::OVERVIEW.get(language))
                    .color(SETTINGS_HEADING)
                    .show(ui);
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let (label, icon, variant) = if self.window_always_on_top {
                    (
                        text::UNPIN_WINDOW.get(language),
                        LucideIcon::PinOff,
                        ButtonVariant::Default,
                    )
                } else {
                    (
                        text::PIN_WINDOW.get(language),
                        LucideIcon::Pin,
                        ButtonVariant::Outline,
                    )
                };
                if ShadcnButton::new(label)
                    .icon(icon)
                    .variant(variant)
                    .selected(self.window_always_on_top)
                    .show(ui)
                    .on_hover_text(if self.window_always_on_top {
                        text::UNPIN_WINDOW_HINT.get(language)
                    } else {
                        text::PIN_WINDOW_HINT.get(language)
                    })
                    .clicked()
                {
                    self.window_always_on_top = !self.window_always_on_top;
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::WindowLevel(
                            if self.window_always_on_top {
                                egui::WindowLevel::AlwaysOnTop
                            } else {
                                egui::WindowLevel::Normal
                            },
                        ));
                }
            });
        });
        ui.add_space(UI_SPACE_4);
        ui.columns(3, |columns| {
            dashboard_stat(
                &mut columns[0],
                text::LIVE_DPS.get(language),
                &snapshot.realtime_dps_text(),
                METRIC_LIVE_DPS,
            );
            columns[0].add_space(UI_SPACE_2);
            dashboard_stat(
                &mut columns[0],
                text::AVERAGE_DPS_30S.get(language),
                &snapshot.average_dps_text(),
                METRIC_AVERAGE_DPS,
            );
            dashboard_stat(
                &mut columns[1],
                text::ROUND_EFFECTIVE_DPS.get(language),
                &snapshot.round_effective_dps_text(),
                METRIC_ACTIVE_DPS,
            );
            columns[1].add_space(UI_SPACE_2);
            dashboard_stat(
                &mut columns[1],
                text::ROUND_BURST_10S.get(language),
                &snapshot.round_burst_10s_dps_text(),
                METRIC_BEST_DPS,
            );
            dashboard_stat(
                &mut columns[2],
                text::ROUND_DAMAGE_TAKEN.get(language),
                &snapshot.round_damage_taken.to_string(),
                METRIC_DAMAGE_TAKEN,
            );
            columns[2].add_space(UI_SPACE_2);
            dashboard_stat(
                &mut columns[2],
                text::BOSS_LOCK.get(language),
                snapshot.boss_lock.as_deref().unwrap_or("-"),
                METRIC_BOSS_LOCK,
            );
        });
        ui.add_space(UI_SPACE_3);
        let chart_round_context = dps_chart_round_context(snapshot, language);
        section_card_with_status(
            ui,
            text::SESSION_DPS_CHART.get(language),
            None,
            chart_round_context
                .as_ref()
                .map(|(_, title)| title.as_str()),
            |ui| {
                dps_history_chart(ui, snapshot, &mut self.dps_chart_view, language);
            },
        );
        ui.add_space(UI_SPACE_3);
        if let Some(report) = &snapshot.round_report {
            section_card(ui, text::PREVIOUS_ROUND_REPORT.get(language), None, |ui| {
                let stats = [
                    ReportStatItem {
                        label: text::DURATION.get(language),
                        value: report.duration_text(),
                        color: METRIC_DURATION,
                    },
                    ReportStatItem {
                        label: text::TOTAL_DAMAGE.get(language),
                        value: report.total_damage.to_string(),
                        color: METRIC_TOTAL_DAMAGE,
                    },
                    ReportStatItem {
                        label: text::EFFECTIVE_DPS.get(language),
                        value: report.effective_dps_text(),
                        color: METRIC_ACTIVE_DPS,
                    },
                    ReportStatItem {
                        label: text::BURST_10S.get(language),
                        value: report.burst_10s_dps_text(),
                        color: METRIC_BEST_DPS,
                    },
                    ReportStatItem {
                        label: text::EFFECTIVE_DPS_GROWTH.get(language),
                        value: if report.has_dps_growth_rate {
                            format!("{}%", report.dps_growth_rate_text())
                        } else {
                            "-".to_owned()
                        },
                        color: if report.dps_growth_rate >= 0.0 {
                            METRIC_DPS_GROWTH
                        } else {
                            METRIC_DAMAGE_TAKEN
                        },
                    },
                    ReportStatItem {
                        label: text::DAMAGE_TAKEN.get(language),
                        value: report.damage_taken.to_string(),
                        color: METRIC_DAMAGE_TAKEN,
                    },
                    ReportStatItem {
                        label: text::LONGEST_STANDSTILL.get(language),
                        value: localized_standstill(report, language),
                        color: METRIC_STANDSTILL,
                    },
                ];
                report_stat_group(ui, &stats, 3);
            });
            ui.add_space(UI_SPACE_3);
        }
        section_card(ui, text::GAME_LOG.get(language), None, |ui| {
            log_source_row(ui, snapshot.source.as_deref(), &self.runtime, language);
        });
    }

    fn open_away_dialog(&mut self) {
        if self.runtime.shared.away_session().is_none() {
            self.away_dialog.reason = AwayReason::Restroom;
            self.away_dialog.duration = AwayDuration::Three;
            self.away_dialog.custom_message = self.persisted.away_custom_message.clone();
        }
        self.away_dialog.open = true;
    }

    fn persist_away_custom_message(&mut self) -> bool {
        if self.away_dialog.custom_message.trim().is_empty() {
            return false;
        }
        if self.away_dialog.custom_message == self.persisted.away_custom_message {
            return true;
        }

        let language = self.draft.language;
        let custom_message = self.away_dialog.custom_message.clone();
        let mut committed = self.persisted.clone();
        committed.version = config::CONFIG_VERSION;
        committed.away_custom_message = custom_message.clone();
        match config::save_atomic(&committed) {
            Ok(()) => {
                self.persisted.away_custom_message = custom_message.clone();
                self.draft.away_custom_message = custom_message.clone();
                let mut live = self.runtime.shared.config.write();
                live.value.away_custom_message = custom_message;
                live.revision = live.revision.wrapping_add(1);
                true
            }
            Err(error) => {
                let detail = format!("{}: {error:#}", text::SAVE_FAILED.get(language));
                self.save_result = Some((detail.clone(), false));
                self.save_error_detail = Some(detail);
                self.save_error_detail_open = true;
                false
            }
        }
    }

    fn show_away_dialog(&mut self, ctx: &egui::Context) {
        if !self.away_dialog.open && !self.away_dialog.exit_confirm_open {
            return;
        }

        let language = self.draft.language;
        let active_session = self.runtime.shared.away_session();
        let is_active = active_session.is_some();
        let applied_osc_enabled = self.runtime.shared.config.read().value.osc_enabled;
        let osc_ready = self.draft.osc_enabled && applied_osc_enabled;
        #[allow(deprecated)]
        let screen_height = ctx.input(|input| input.screen_rect().height());
        let content_height = (screen_height * 0.8 - 112.0).max(160.0);
        const AWAY_DIALOG_CONTENT_MIN_HEIGHT: f32 = 410.0;
        let mut open = self.away_dialog.open;
        let was_open = open;
        let mut start_clicked = false;
        let mut stop_clicked = false;

        Dialog::new()
            .title(text::AWAY_MODE.get(language))
            .close_label(text::CLOSE_DIALOG.get(language))
            .width(620.0)
            .close_on_backdrop(false)
            .close_on_escape(false)
            .show(ctx, &mut open, |ui| {
                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), content_height),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ShadcnScrollArea::new(content_height)
                            .id_salt("away-mode-dialog")
                            .framed(false)
                            .fill_available(true)
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                let layout_height =
                                    AWAY_DIALOG_CONTENT_MIN_HEIGHT.max(content_height);
                                ui.allocate_ui_with_layout(
                                    egui::vec2(ui.available_width(), layout_height),
                                    egui::Layout::top_down(egui::Align::Min),
                                    |ui| {
                                        ui.columns(2, |columns| {
                                            columns[0].add_enabled_ui(!is_active, |ui| {
                                                Typography::small(text::AWAY_REASON.get(language))
                                                    .color(SETTINGS_TEXT_MUTED)
                                                    .show(ui);
                                                ui.add_space(UI_SPACE_2);
                                                let reasons = [
                                                    AwayReasonChoice(
                                                        AwayReason::Restroom,
                                                        language,
                                                    ),
                                                    AwayReasonChoice(AwayReason::Takeout, language),
                                                    AwayReasonChoice(AwayReason::Custom, language),
                                                ];
                                                let mut selected = AwayReasonChoice(
                                                    self.away_dialog.reason,
                                                    language,
                                                );
                                                let select_width = ui.available_width();
                                                let response =
                                                    SelectValue::new(&mut selected, &reasons)
                                                        .width(select_width)
                                                        .show(ui);
                                                if response.changed()
                                                    || selected.0 != self.away_dialog.reason
                                                {
                                                    self.away_dialog.reason = selected.0;
                                                }
                                            });

                                            columns[1].add_enabled_ui(!is_active, |ui| {
                                                Typography::small(
                                                    text::AWAY_DURATION.get(language),
                                                )
                                                .color(SETTINGS_TEXT_MUTED)
                                                .show(ui);
                                                ui.add_space(UI_SPACE_2);
                                                let durations = AwayDuration::ALL.map(|duration| {
                                                    AwayDurationChoice(duration, language)
                                                });
                                                let mut selected = AwayDurationChoice(
                                                    self.away_dialog.duration,
                                                    language,
                                                );
                                                let select_width = ui.available_width();
                                                let response =
                                                    SelectValue::new(&mut selected, &durations)
                                                        .width(select_width)
                                                        .show(ui);
                                                if response.changed()
                                                    || selected.0 != self.away_dialog.duration
                                                {
                                                    self.away_dialog.duration = selected.0;
                                                }
                                            });
                                        });

                                        ui.add_space(UI_SPACE_4);
                                        let preview = active_session.as_ref().map_or_else(
                                            || {
                                                render_away_preview(
                                                    self.away_dialog.reason,
                                                    &self.away_dialog.custom_message,
                                                    self.away_dialog.duration,
                                                    language,
                                                )
                                            },
                                            |session| {
                                                ecliptica_data_analyzer::osc::render_away_message(
                                                    session,
                                                    language,
                                                    Instant::now(),
                                                )
                                            },
                                        );

                                        if self.away_dialog.reason == AwayReason::Custom {
                                            Typography::small(
                                                text::AWAY_CUSTOM_MESSAGE.get(language),
                                            )
                                            .color(SETTINGS_TEXT_MUTED)
                                            .show(ui);
                                            ui.add_space(UI_SPACE_2);
                                            let editor_width =
                                                (ui.available_width() - 24.0).max(120.0);
                                            ui.allocate_ui_with_layout(
                                                egui::vec2(editor_width, 140.0),
                                                egui::Layout::top_down(egui::Align::Min),
                                                |ui| {
                                                    ui.add_enabled_ui(!is_active, |ui| {
                                                        Textarea::new(
                                                            &mut self.away_dialog.custom_message,
                                                        )
                                                        .id_salt("away-custom-message")
                                                        .desired_width(ui.available_width())
                                                        .min_height(140.0)
                                                        .max_height(140.0)
                                                        .show(ui);
                                                    });
                                                },
                                            );

                                            ui.add_space(UI_SPACE_2);
                                            let custom_missing =
                                                self.away_dialog.custom_message.trim().is_empty();
                                            Typography::small(if custom_missing {
                                                text::AWAY_MESSAGE_REQUIRED.get(language)
                                            } else {
                                                text::AWAY_TIME_VARIABLE_HINT.get(language)
                                            })
                                            .color(if custom_missing {
                                                SETTINGS_DANGER
                                            } else {
                                                SETTINGS_TEXT_MUTED
                                            })
                                            .show(ui);

                                            ui.add_space(UI_SPACE_3);
                                        }

                                        Typography::new(text::AWAY_MESSAGE.get(language))
                                            .strong()
                                            .color(SETTINGS_HEADING)
                                            .show(ui);
                                        ui.add_space(UI_SPACE_2);
                                        let preview_width = ui.available_width();
                                        egui::Frame::NONE
                                            .fill(SETTINGS_BG)
                                            .inner_margin(egui::Margin::same(12))
                                            .corner_radius(6.0)
                                            .show(ui, |ui| {
                                                ui.set_width((preview_width - 24.0).max(1.0));
                                                preview_text(ui, &preview);
                                            });

                                        let footer_height = 32.0;
                                        ui.add_space(
                                            (ui.available_height() - footer_height).max(16.0),
                                        );
                                        ui.horizontal(|ui| {
                                            ui.allocate_ui_with_layout(
                                                egui::vec2(
                                                    (ui.available_width() - 190.0).max(1.0),
                                                    footer_height,
                                                ),
                                                egui::Layout::left_to_right(egui::Align::Center),
                                                |ui| {
                                                    if is_active {
                                                        Badge::new(
                                                            text::AWAY_MODE_ACTIVE.get(language),
                                                        )
                                                        .variant(BadgeVariant::Success)
                                                        .show(ui);
                                                    } else if !osc_ready {
                                                        Typography::small(
                                                            text::AWAY_OSC_REQUIRED.get(language),
                                                        )
                                                        .color(SETTINGS_TEXT_MUTED)
                                                        .show(ui);
                                                    }
                                                },
                                            );

                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    if is_active {
                                                        stop_clicked = ShadcnButton::new(
                                                            text::EXIT_AWAY_MODE.get(language),
                                                        )
                                                        .icon(LucideIcon::TimerOff)
                                                        .variant(ButtonVariant::Destructive)
                                                        .show(ui)
                                                        .clicked();
                                                    } else {
                                                        let custom_valid = self.away_dialog.reason
                                                            != AwayReason::Custom
                                                            || !self
                                                                .away_dialog
                                                                .custom_message
                                                                .trim()
                                                                .is_empty();
                                                        start_clicked = ShadcnButton::new(
                                                            text::ENTER_AWAY_MODE.get(language),
                                                        )
                                                        .icon(LucideIcon::Timer)
                                                        .enabled(osc_ready && custom_valid)
                                                        .show(ui)
                                                        .clicked();
                                                    }
                                                },
                                            );
                                        });
                                    },
                                );
                            });
                    },
                );
            });

        if stop_clicked {
            self.runtime.shared.stop_away_mode();
        }
        if start_clicked
            && (self.away_dialog.reason != AwayReason::Custom || self.persist_away_custom_message())
        {
            self.runtime.shared.start_away_mode(
                self.away_dialog.reason,
                self.away_dialog.custom_message.clone(),
                self.away_dialog.duration.duration(),
            );
        }

        if was_open && !open {
            if is_active {
                self.away_dialog.open = true;
                self.away_dialog.exit_confirm_open = true;
            } else {
                self.persist_away_custom_message();
                self.away_dialog.open = false;
            }
        } else {
            self.away_dialog.open = open;
        }

        match AlertDialog::new(
            text::EXIT_AWAY_MODE_TITLE.get(language),
            text::EXIT_AWAY_MODE_DESCRIPTION.get(language),
        )
        .close_label(text::CLOSE_DIALOG.get(language))
        .cancel_text(text::KEEP_AWAY_MODE.get(language))
        .action_text(text::CONFIRM_EXIT_AWAY_MODE.get(language))
        .destructive()
        .close_on_escape(false)
        .show(ctx, &mut self.away_dialog.exit_confirm_open)
        {
            AlertDialogResult::Confirmed => {
                self.runtime.shared.stop_away_mode();
                self.away_dialog.open = false;
            }
            AlertDialogResult::Cancelled => {
                self.away_dialog.open = true;
            }
            AlertDialogResult::Open => {}
        }
    }

    fn message_page(&mut self, ui: &mut egui::Ui, snapshot: &GameSnapshot) {
        let language = self.draft.language;
        let (
            applied_message_preset,
            applied_report_preset,
            message_draft_changed,
            report_draft_changed,
        ) = {
            let applied = self.runtime.shared.config.read();
            (
                applied.value.active_message_template_preset,
                applied.value.active_round_report_template_preset,
                message_template_draft_changed(&self.draft, &applied.value),
                report_template_draft_changed(&self.draft, &applied.value),
            )
        };
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                Typography::h3(text::OSC_MESSAGES.get(language))
                    .color(SETTINGS_HEADING)
                    .show(ui);
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ShadcnButton::new(text::OPEN_AWAY_MODE.get(language))
                    .icon(LucideIcon::DoorOpen)
                    .variant(ButtonVariant::Outline)
                    .show(ui)
                    .clicked()
                {
                    self.open_away_dialog();
                }
            });
        });
        ui.add_space(UI_SPACE_4);
        section_card(
            ui,
            text::SEND_SETTINGS.get(self.draft.language),
            Some(text::SEND_SETTINGS_DESCRIPTION.get(self.draft.language)),
            |ui| {
                Switch::new(&mut self.draft.osc_enabled)
                    .label(text::ENABLE_OSC.get(self.draft.language))
                    .show(ui);
                ui.add_space(UI_SPACE_2);
                let options = [
                    SendIntervalChoice(SendInterval::One, self.draft.language),
                    SendIntervalChoice(SendInterval::OnePointFive, self.draft.language),
                    SendIntervalChoice(SendInterval::Two, self.draft.language),
                    SendIntervalChoice(SendInterval::Three, self.draft.language),
                ];
                let mut selected =
                    SendIntervalChoice(self.draft.send_interval, self.draft.language);
                PropertyRow::new(text::SEND_INTERVAL.get(self.draft.language)).show(ui, |ui| {
                    let response = SelectValue::new(&mut selected, &options)
                        .width(190.0)
                        .show(ui);
                    if response.changed() || selected.0 != self.draft.send_interval {
                        self.draft.send_interval = selected.0;
                    }
                });
                PropertyRow::new(text::TARGET_ADDRESS.get(self.draft.language)).show(ui, |ui| {
                    Input::new(&mut self.draft.osc_address)
                        .placeholder("127.0.0.1:9000")
                        .desired_width(190.0)
                        .show(ui);
                });
            },
        );
        ui.add_space(UI_SPACE_3);
        section_card(
            ui,
            text::NORMAL_MESSAGE_TEMPLATE.get(language),
            None,
            |ui| {
                PropertyRow::new(text::TEMPLATE_PRESET.get(language))
                    .label_width(84.0)
                    .show(ui, |ui| {
                        let mut selected = self.draft.active_message_template_preset;
                        let reset_clicked = preset_controls_row(ui, |ui| {
                            ToggleGroup::new(preset_tab_labels(
                                &self.draft.message_template_preset_names,
                                language,
                            ))
                            .variant(ToggleVariant::Outline)
                            .size(ComponentSize::Xs)
                            .strong_labels(false)
                            .applied_index(applied_message_preset)
                            .draft_changed(message_draft_changed)
                            .show(ui, &mut selected);
                            ui.add_space(UI_SPACE_2);
                            ShadcnButton::new(text::RESET_SELECTED_PRESET.get(language))
                                .icon(LucideIcon::RotateCcw)
                                .variant(ButtonVariant::Ghost)
                                .size(ComponentSize::Xs)
                                .height(TEMPLATE_PRESET_TAB_ROW_HEIGHT)
                                .show(ui)
                                .on_hover_text(text::RESET_MESSAGE_PRESET_HINT.get(language))
                                .clicked()
                        })
                        .inner;
                        if reset_clicked {
                            self.template_preset_reset_confirm =
                                Some(TemplatePresetResetKind::Message);
                        }
                        if selected != self.draft.active_message_template_preset {
                            self.draft.select_message_template_preset(selected);
                            let name = preset_display_name(
                                &self.draft.message_template_preset_names[selected],
                                selected,
                                language,
                            );
                            self.save_result = Some((
                                format_pattern(text::PRESET_SWITCHED, language, &[("name", name)]),
                                true,
                            ));
                        }
                    });
                PropertyRow::new(text::PRESET_NAME.get(language))
                    .label_width(84.0)
                    .show(ui, |ui| {
                        let active = self.draft.active_message_template_preset;
                        let name = &mut self.draft.message_template_preset_names[active];
                        let response = Input::new(name)
                            .placeholder(format_pattern(
                                text::PRESET_FALLBACK,
                                language,
                                &[("index", (active + 1).to_string())],
                            ))
                            .desired_width(260.0)
                            .show(ui)
                            .on_hover_text(format_pattern(
                                text::PRESET_NAME_HINT,
                                language,
                                &[("max", config::TEMPLATE_PRESET_NAME_MAX_CHARS.to_string())],
                            ));
                        if response.lost_focus() {
                            *name = name.trim().to_owned();
                        }
                    });
                ui.add_space(UI_SPACE_2);
                let width = ui.available_width();
                Textarea::new(&mut self.draft.message_template)
                    .id_salt("osc-message-template")
                    .desired_width(width)
                    .min_height(178.0)
                    .auto_resize()
                    .max_height(360.0)
                    .monospace()
                    .show(ui);
                ui.add_space(UI_SPACE_2);
                Typography::muted(text::LIVE_VARIABLES_HINT.get(language)).show(ui);
                template_help_button(ui, &mut self.template_help_open, language);
                ui.add_space(UI_SPACE_2);
                let clipboard = &mut self.clipboard;
                let toast_state = &mut self.toast_state;
                live_variable_help(
                    ui,
                    clipboard,
                    toast_state,
                    language,
                    snapshot.has_heart_rate,
                );
            },
        );
        ui.add_space(UI_SPACE_3);
        section_card(ui, text::ROUND_REPORT_TEMPLATE.get(language), None, |ui| {
            PropertyRow::new(text::REPORT_PRESET.get(language))
                .label_width(84.0)
                .show(ui, |ui| {
                    let mut selected = self.draft.active_round_report_template_preset;
                    let reset_clicked = preset_controls_row(ui, |ui| {
                        ToggleGroup::new(preset_tab_labels(
                            &self.draft.round_report_template_preset_names,
                            language,
                        ))
                        .variant(ToggleVariant::Outline)
                        .size(ComponentSize::Xs)
                        .strong_labels(false)
                        .applied_index(applied_report_preset)
                        .draft_changed(report_draft_changed)
                        .show(ui, &mut selected);
                        ui.add_space(UI_SPACE_2);
                        ShadcnButton::new(text::RESET_SELECTED_PRESET.get(language))
                            .icon(LucideIcon::RotateCcw)
                            .variant(ButtonVariant::Ghost)
                            .size(ComponentSize::Xs)
                            .height(TEMPLATE_PRESET_TAB_ROW_HEIGHT)
                            .show(ui)
                            .on_hover_text(text::RESET_REPORT_PRESET_HINT.get(language))
                            .clicked()
                    })
                    .inner;
                    if reset_clicked {
                        self.template_preset_reset_confirm = Some(TemplatePresetResetKind::Report);
                    }
                    if selected != self.draft.active_round_report_template_preset {
                        self.draft.select_round_report_template_preset(selected);
                        let name = preset_display_name(
                            &self.draft.round_report_template_preset_names[selected],
                            selected,
                            language,
                        );
                        self.save_result = Some((
                            format_pattern(text::PRESET_SWITCHED, language, &[("name", name)]),
                            true,
                        ));
                    }
                });
            PropertyRow::new(text::PRESET_NAME.get(language))
                .label_width(84.0)
                .show(ui, |ui| {
                    let active = self.draft.active_round_report_template_preset;
                    let name = &mut self.draft.round_report_template_preset_names[active];
                    let response = Input::new(name)
                        .placeholder(format_pattern(
                            text::PRESET_FALLBACK,
                            language,
                            &[("index", (active + 1).to_string())],
                        ))
                        .desired_width(260.0)
                        .show(ui)
                        .on_hover_text(format_pattern(
                            text::PRESET_NAME_HINT,
                            language,
                            &[("max", config::TEMPLATE_PRESET_NAME_MAX_CHARS.to_string())],
                        ));
                    if response.lost_focus() {
                        *name = name.trim().to_owned();
                    }
                });
            ui.add_space(UI_SPACE_2);
            let width = ui.available_width();
            Textarea::new(&mut self.draft.round_report_template)
                .id_salt("osc-round-report-template")
                .desired_width(width)
                .min_height(118.0)
                .auto_resize()
                .max_height(360.0)
                .monospace()
                .show(ui);
            ui.add_space(UI_SPACE_2);
            Typography::muted(text::REPORT_VARIABLES_HINT.get(language)).show(ui);
            template_help_button(ui, &mut self.template_help_open, language);
            ui.add_space(UI_SPACE_2);
            let clipboard = &mut self.clipboard;
            let toast_state = &mut self.toast_state;
            report_variable_help(
                ui,
                clipboard,
                toast_state,
                language,
                snapshot.has_heart_rate,
            );
        });
        ui.add_space(UI_SPACE_3);
        section_card(ui, text::LIVE_PREVIEW.get(language), None, |ui| {
            PropertyRow::new(text::SIMULATED_STATE.get(language))
                .label_width(84.0)
                .show(ui, |ui| {
                    let mut selected = match self.template_preview_state {
                        TemplatePreviewState::Normal => 0,
                        TemplatePreviewState::RoundReport => 1,
                    };
                    ToggleGroup::new(vec![
                        text::PREVIEW_NORMAL.get(language).to_owned(),
                        text::PREVIEW_ROUND_REPORT.get(language).to_owned(),
                    ])
                    .variant(ToggleVariant::Outline)
                    .size(ComponentSize::Xs)
                    .selection_markers(false)
                    .show(ui, &mut selected);
                    self.template_preview_state = match selected {
                        1 => TemplatePreviewState::RoundReport,
                        _ => TemplatePreviewState::Normal,
                    };
                });
            ui.add_space(UI_SPACE_2);
            let preview_snapshot =
                preview_snapshot_for_state(snapshot, self.template_preview_state);
            match ecliptica_data_analyzer::osc::render_configured_message(
                &self.draft,
                &preview_snapshot,
            ) {
                Ok(preview) => {
                    preview_panel(ui, |ui| {
                        if preview.trim().is_empty() {
                            Typography::new(text::EMPTY_MESSAGE.get(language))
                                .italics()
                                .show(ui);
                        } else {
                            preview_text(ui, &preview);
                        }
                    });
                }
                Err(error) => {
                    Alert::new()
                        .variant(AlertVariant::Destructive)
                        .full_width()
                        .show(ui, |ui| {
                            Typography::new(format!(
                                "{}: {error}",
                                text::TEMPLATE_ERROR.get(language)
                            ))
                            .color(egui::Color32::from_rgb(255, 132, 146))
                            .show(ui);
                        });
                }
            }
        });
        ui.add_space(UI_SPACE_3);
        heart_rate_auxiliary_panel(
            ui,
            &mut self.draft.heart_rate_enabled,
            &mut self.clipboard,
            &mut self.toast_state,
            language,
            snapshot.has_heart_rate,
        );
    }

    fn player_page(&mut self, ui: &mut egui::Ui) {
        page_heading(ui, text::PLAYER_ALERTS.get(self.draft.language));
        section_card(
            ui,
            text::PLAYER_IDENTITY.get(self.draft.language),
            Some(text::PLAYER_IDENTITY_DESCRIPTION.get(self.draft.language)),
            |ui| {
                let width = ui.available_width();
                Input::new(&mut self.draft.display_name)
                    .placeholder(text::DISPLAY_NAME_PLACEHOLDER.get(self.draft.language))
                    .desired_width(width)
                    .show(ui);
            },
        );
        ui.add_space(UI_SPACE_3);
        section_card(
            ui,
            text::ALERT_SOUNDS.get(self.draft.language),
            None,
            |ui| {
                PropertyRow::new(text::VOLUME.get(self.draft.language)).show(ui, |ui| {
                    Flex::row().align_center().gap(UI_SPACE_2).show(ui, |flex| {
                        flex.ui(|ui| {
                            ShadcnSlider::f32(&mut self.draft.alert_volume, 0.0..=1.0)
                                .label(text::ALERT_VOLUME.get(self.draft.language))
                                .width(240.0)
                                .show(ui);
                        });
                        flex.ui(|ui| {
                            Typography::new(format!("{:.0}%", self.draft.alert_volume * 100.0))
                                .monospace()
                                .color(SETTINGS_ACCENT)
                                .show(ui);
                        });
                    });
                });
                PropertyRow::new(text::LOCK_SOUND.get(self.draft.language))
                    .label_width(alert_sound_label_width(self.draft.language))
                    .show(ui, |ui| {
                        let options = AlertSoundStyle::ALL
                            .map(|style| AlertSoundStyleChoice(style, self.draft.language));
                        let mut selected = AlertSoundStyleChoice(
                            self.draft.locked_sound_style,
                            self.draft.language,
                        );
                        Flex::row().align_center().gap(8.0).wrap().show(ui, |flex| {
                            flex.ui(|ui| {
                                let response = SelectValue::new(&mut selected, &options)
                                    .width(150.0)
                                    .show(ui);
                                if response.changed() || selected.0 != self.draft.locked_sound_style
                                {
                                    self.draft.locked_sound_style = selected.0;
                                }
                            });
                            flex.ui(|ui| {
                                if ShadcnButton::new(text::PREVIEW_SOUND.get(self.draft.language))
                                    .icon(LucideIcon::Volume2)
                                    .variant(ButtonVariant::Outline)
                                    .show(ui)
                                    .clicked()
                                {
                                    let _ =
                                        self.runtime.sounds.try_send(SoundCommand::PreviewLocked(
                                            self.draft.alert_volume,
                                            self.draft.locked_sound_style,
                                        ));
                                }
                            });
                        });
                    });
                PropertyRow::new(text::RELEASE_SOUND.get(self.draft.language))
                    .label_width(alert_sound_label_width(self.draft.language))
                    .show(ui, |ui| {
                        let options = AlertSoundStyle::ALL
                            .map(|style| AlertSoundStyleChoice(style, self.draft.language));
                        let mut selected = AlertSoundStyleChoice(
                            self.draft.unlocked_sound_style,
                            self.draft.language,
                        );
                        Flex::row().align_center().gap(8.0).wrap().show(ui, |flex| {
                            flex.ui(|ui| {
                                let response = SelectValue::new(&mut selected, &options)
                                    .width(150.0)
                                    .show(ui);
                                if response.changed()
                                    || selected.0 != self.draft.unlocked_sound_style
                                {
                                    self.draft.unlocked_sound_style = selected.0;
                                }
                            });
                            flex.ui(|ui| {
                                if ShadcnButton::new(text::PREVIEW_SOUND.get(self.draft.language))
                                    .icon(LucideIcon::VolumeX)
                                    .variant(ButtonVariant::Outline)
                                    .show(ui)
                                    .clicked()
                                {
                                    let _ = self.runtime.sounds.try_send(
                                        SoundCommand::PreviewUnlocked(
                                            self.draft.alert_volume,
                                            self.draft.unlocked_sound_style,
                                        ),
                                    );
                                }
                            });
                        });
                    });
            },
        );
    }

    fn overlay_page(&mut self, ui: &mut egui::Ui) {
        let language = self.draft.language;
        page_heading(ui, text::OVERLAY.get(language));
        section_card(ui, text::WINDOW_BEHAVIOR.get(language), None, |ui| {
            let mut draggable = self.draft.overlay_draggable();
            let response = Switch::new(&mut draggable)
                .label(text::DRAGGABLE.get(language))
                .show(ui);
            if response.changed() {
                self.draft.set_overlay_draggable(draggable);
                self.runtime
                    .shared
                    .config
                    .write()
                    .value
                    .set_overlay_draggable(draggable);
                let overlay_id = egui::ViewportId::from_hash_of(OVERLAY_ID);
                ui.ctx().send_viewport_cmd_to(
                    overlay_id,
                    egui::ViewportCommand::MousePassthrough(!draggable),
                );
                if draggable {
                    // StartDrag is ignored by egui-winit while a viewport is
                    // unfocused. Focus once when drag mode is enabled so the
                    // first press anywhere on the Overlay can move the window.
                    ui.ctx()
                        .send_viewport_cmd_to(overlay_id, egui::ViewportCommand::Focus);
                }
            }
            if draggable {
                ui.add_space(UI_SPACE_2);
                Typography::new(text::DRAG_OVERLAY_HINT.get(language))
                    .color(SETTINGS_INFO)
                    .show(ui);
            }
            ui.add_space(UI_SPACE_2);
            let options = OVERLAY_SCALE_OPTIONS.map(OverlayScaleChoice);
            let mut selected = OverlayScaleChoice(self.draft.overlay_scale);
            PropertyRow::new(text::OVERLAY_SIZE.get(language)).show(ui, |ui| {
                let response = SelectValue::new(&mut selected, &options)
                    .width(150.0)
                    .show(ui);
                if response.changed() || selected.0 != self.draft.overlay_scale {
                    self.draft.overlay_scale = selected.0;
                    self.runtime.shared.config.write().value.overlay_scale = selected.0;
                }
            });
        });
        ui.add_space(UI_SPACE_3);
        section_card(ui, text::SCREEN_POSITION.get(language), None, |ui| {
            PropertyRow::new(text::HORIZONTAL_POSITION.get(language)).show(ui, |ui| {
                let response = NumberInput::f32(&mut self.draft.overlay_x)
                    .speed(1.0)
                    .range(0.0..=10000.0)
                    .decimals(0)
                    .suffix(text::PIXELS_SUFFIX.get(language))
                    .width(150.0)
                    .show(ui);
                if response.changed() {
                    let mut live = self.runtime.shared.config.write();
                    live.value.overlay_x = self.draft.overlay_x;
                }
            });
            PropertyRow::new(text::VERTICAL_POSITION.get(language)).show(ui, |ui| {
                let response = NumberInput::f32(&mut self.draft.overlay_y)
                    .speed(1.0)
                    .range(0.0..=10000.0)
                    .decimals(0)
                    .suffix(text::PIXELS_SUFFIX.get(language))
                    .width(150.0)
                    .show(ui);
                if response.changed() {
                    let mut live = self.runtime.shared.config.write();
                    live.value.overlay_y = self.draft.overlay_y;
                }
            });
        });
    }

    fn logs_page(&mut self, ui: &mut egui::Ui) {
        let language = self.draft.language;
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                Typography::h3(text::SYSTEM_LOGS.get(language))
                    .color(SETTINGS_HEADING)
                    .show(ui);
            });
            if !self.developer_mode {
                return;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ShadcnButton::new(text::DEVELOPER_LOGS.get(language))
                    .icon(LucideIcon::Bug)
                    .variant(ButtonVariant::Ghost)
                    .size(ComponentSize::Xs)
                    .show(ui)
                    .on_hover_text(text::DEVELOPER_LOGS_HINT.get(language))
                    .clicked()
                {
                    self.developer_logs_open = true;
                }
            });
        });
        ui.add_space(UI_SPACE_4);
        section_card(ui, text::EVENT_STREAM.get(language), None, |ui| {
            inset_surface(ui, SETTINGS_PREVIEW_BG, |ui| {
                for row in &self.logs {
                    log_line(ui, row, language);
                }
                if self.logs.is_empty() {
                    Empty::show(ui, |ui| {
                        Typography::muted(text::NO_SYSTEM_EVENTS.get(language)).show(ui);
                    });
                }
            });
        });
    }

    fn overlay_ui(&self, ctx: &egui::Context, snapshot: &GameSnapshot) {
        let config = self.runtime.shared.config.read().value.clone();
        let language = config.language;
        let overlay_draggable = config.overlay_draggable();
        let overlay_scale = config.overlay_scale;
        let has_alert = self.alert.is_some();
        let overlay_height =
            overlay_height(snapshot.round_report.is_some(), has_alert) * overlay_scale;
        let overlay_width = OVERLAY_WIDTH * overlay_scale;
        let builder = egui::ViewportBuilder::default()
            .with_title(overlay_window_title(language))
            .with_inner_size([overlay_width, overlay_height])
            .with_min_inner_size([overlay_width, overlay_height])
            .with_max_inner_size([overlay_width, overlay_height])
            .with_position([config.overlay_x, config.overlay_y])
            .with_decorations(false)
            .with_transparent(true)
            .with_resizable(false)
            .with_taskbar(false)
            .with_always_on_top()
            .with_mouse_passthrough(!overlay_draggable)
            .with_movable_by_background(overlay_draggable)
            .with_has_shadow(false);
        let alert = self.alert.clone();
        let display_name = config.display_name.clone();
        let snapshot = snapshot.clone();
        let overlay_position = Arc::clone(&self.overlay_position);
        ctx.show_viewport_deferred(
            egui::ViewportId::from_hash_of(OVERLAY_ID),
            builder,
            move |overlay_ctx, _class| {
                let overlay_panel = egui::CentralPanel::default()
                    .frame(
                        egui::Frame::new()
                            .fill(egui::Color32::TRANSPARENT)
                            .inner_margin(8.0 * overlay_scale),
                    )
                    .show(overlay_ctx, |ui| {
                        egui::Frame::new()
                            .fill(egui::Color32::from_rgba_unmultiplied(12, 17, 29, 176))
                            .corner_radius(18.0 * overlay_scale)
                            .inner_margin(14.0 * overlay_scale)
                            .stroke(egui::Stroke::new(
                                1.0 * overlay_scale,
                                egui::Color32::from_rgba_unmultiplied(120, 170, 220, 72),
                            ))
                            .show(ui, |ui| {
                                // The viewport is fixed-size. Clamp the content UI as well so
                                // child cards can truncate but can never grow the Overlay.
                                ui.set_width(
                                    (OVERLAY_CONTENT_WIDTH * overlay_scale)
                                        .min(ui.available_width()),
                                );
                                let spacing = ui.spacing_mut();
                                spacing.item_spacing = OVERLAY_ITEM_SPACING * overlay_scale;
                                spacing.interact_size *= overlay_scale;
                                overlay_header(
                                    ui,
                                    snapshot.has_heart_rate.then_some(snapshot.heart_rate),
                                    overlay_scale,
                                    language,
                                );
                                ui.add_space(9.0 * overlay_scale);
                                if let Some(report) = &snapshot.round_report {
                                    ui.horizontal(|ui| {
                                        ui.colored_label(
                                            METRIC_BEST_DPS,
                                            egui::RichText::new(
                                                text::ROUND_REPORT_HEADING.get(language),
                                            )
                                            .strong()
                                            .size(12.0 * overlay_scale),
                                        );
                                        ui.label(
                                            egui::RichText::new(
                                                text::RETURNED_TO_LOBBY.get(language),
                                            )
                                            .size(11.0 * overlay_scale)
                                            .color(TEXT_SECONDARY),
                                        );
                                    });
                                    ui.add_space(7.0 * overlay_scale);
                                    let duration = report.duration_text();
                                    let total_exact = report.total_damage.to_string();
                                    let total_compact = compact_u64(report.total_damage, language);
                                    let effective_exact = report.effective_dps_text();
                                    let effective_compact = if report.has_output_data {
                                        compact_f64(report.effective_dps, language)
                                    } else {
                                        effective_exact.clone()
                                    };
                                    let burst_exact = report.burst_10s_dps_text();
                                    let burst_compact = report
                                        .burst_10s_dps
                                        .map(|value| compact_f64(value, language))
                                        .unwrap_or_else(|| burst_exact.clone());
                                    let taken_exact = report.damage_taken.to_string();
                                    let taken_compact = compact_u64(report.damage_taken, language);
                                    let standstill = localized_standstill(report, language);
                                    ui.columns(3, |columns| {
                                        overlay_report_stat(
                                            &mut columns[0],
                                            text::TIME_USED.get(language),
                                            &duration,
                                            &duration,
                                            METRIC_DURATION,
                                            overlay_scale,
                                            language,
                                        );
                                        overlay_report_stat(
                                            &mut columns[1],
                                            text::TOTAL_DAMAGE.get(language),
                                            &total_compact,
                                            &total_exact,
                                            METRIC_TOTAL_DAMAGE,
                                            overlay_scale,
                                            language,
                                        );
                                        overlay_report_stat(
                                            &mut columns[2],
                                            text::LONGEST_STANDSTILL.get(language),
                                            &standstill,
                                            &standstill,
                                            METRIC_STANDSTILL,
                                            overlay_scale,
                                            language,
                                        );
                                    });
                                    ui.add_space(6.0 * overlay_scale);
                                    ui.columns(3, |columns| {
                                        overlay_report_stat(
                                            &mut columns[0],
                                            text::EFFECTIVE_DPS.get(language),
                                            &effective_compact,
                                            &effective_exact,
                                            METRIC_ACTIVE_DPS,
                                            overlay_scale,
                                            language,
                                        );
                                        overlay_report_stat(
                                            &mut columns[1],
                                            text::BURST_10S.get(language),
                                            &burst_compact,
                                            &burst_exact,
                                            METRIC_BEST_DPS,
                                            overlay_scale,
                                            language,
                                        );
                                        overlay_report_stat(
                                            &mut columns[2],
                                            text::DAMAGE_TAKEN_SHORT.get(language),
                                            &taken_compact,
                                            &taken_exact,
                                            METRIC_DAMAGE_TAKEN,
                                            overlay_scale,
                                            language,
                                        );
                                    });
                                } else {
                                    let latest_exact = snapshot.latest_dps_text();
                                    let latest_display = if snapshot.has_damage_data {
                                        compact_u64(snapshot.latest_dps, language)
                                    } else {
                                        latest_exact.clone()
                                    };
                                    let effective_exact = snapshot.round_effective_dps_text();
                                    let effective_display = if snapshot.has_damage_data {
                                        compact_f64(snapshot.round_effective_dps, language)
                                    } else {
                                        effective_exact.clone()
                                    };
                                    let burst_exact = snapshot.round_burst_10s_dps_text();
                                    let burst_display = snapshot
                                        .round_burst_10s_dps
                                        .map(|value| compact_f64(value, language))
                                        .unwrap_or_else(|| burst_exact.clone());
                                    let taken_exact = snapshot.round_damage_taken.to_string();
                                    let taken_display =
                                        compact_u64(snapshot.round_damage_taken, language);
                                    ui.columns(4, |columns| {
                                        overlay_stat(
                                            &mut columns[0],
                                            text::LATEST.get(language),
                                            text::LIVE_DPS.get(language),
                                            &latest_display,
                                            &latest_exact,
                                            METRIC_LIVE_DPS,
                                            overlay_scale,
                                            language,
                                        );
                                        overlay_stat(
                                            &mut columns[1],
                                            text::EFFECTIVE.get(language),
                                            text::ROUND_EFFECTIVE_DPS.get(language),
                                            &effective_display,
                                            &effective_exact,
                                            METRIC_ACTIVE_DPS,
                                            overlay_scale,
                                            language,
                                        );
                                        overlay_stat(
                                            &mut columns[2],
                                            text::BURST_10S_SHORT.get(language),
                                            text::ROUND_BURST_10S.get(language),
                                            &burst_display,
                                            &burst_exact,
                                            METRIC_BEST_DPS,
                                            overlay_scale,
                                            language,
                                        );
                                        overlay_stat(
                                            &mut columns[3],
                                            text::DAMAGE_TAKEN_SHORT.get(language),
                                            text::ROUND_DAMAGE_TAKEN_TOTAL.get(language),
                                            &taken_display,
                                            &taken_exact,
                                            METRIC_DAMAGE_TAKEN,
                                            overlay_scale,
                                            language,
                                        );
                                    });
                                    ui.add_space(8.0 * overlay_scale);
                                    let lock = snapshot.boss_lock.as_deref().unwrap_or("-");
                                    let locked_self = !display_name.trim().is_empty()
                                        && normalized_name(lock) == normalized_name(&display_name);
                                    lock_card(ui, lock, locked_self, overlay_scale, language);
                                }

                                if let Some((message, _, level)) = &alert {
                                    ui.add_space(8.0 * overlay_scale);
                                    let alert_color = if *level == EventLevel::Error {
                                        SETTINGS_DANGER
                                    } else {
                                        SETTINGS_WARNING
                                    };
                                    egui::Frame::new()
                                        .fill(alert_color.gamma_multiply(0.12))
                                        .corner_radius(9.0 * overlay_scale)
                                        .inner_margin(egui::Margin::symmetric(
                                            scaled_overlay_margin(10, overlay_scale),
                                            scaled_overlay_margin(7, overlay_scale),
                                        ))
                                        .show(ui, |ui| {
                                            ui.add(
                                                egui::Label::new(
                                                    egui::RichText::new(short_text(message, 52))
                                                        .color(alert_color)
                                                        .size(12.0 * overlay_scale),
                                                )
                                                .wrap(),
                                            );
                                        });
                                }
                            });
                    });
                if overlay_draggable {
                    // Register the drag surface after all child widgets so labels and
                    // value tooltips cannot reduce the native Windows hit target to a
                    // small piece of empty background.
                    let drag = overlay_panel
                        .response
                        .interact(egui::Sense::drag())
                        .on_hover_cursor(egui::CursorIcon::Grab);
                    if drag.drag_started_by(egui::PointerButton::Primary) {
                        overlay_ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                    }
                }
                if let Some(position) =
                    overlay_ctx.input(|input| input.viewport().outer_rect.map(|rect| rect.min))
                {
                    let mut position_state = overlay_position.lock();
                    let changed = position_state
                        .last_observed
                        .is_none_or(|previous| previous.distance(position) >= 0.5);
                    if changed {
                        position_state.last_observed = Some(position);
                        position_state.pending = Some(position);
                    }
                }
                overlay_ctx.request_repaint_after(Duration::from_millis(250));
            },
        );
    }
}

fn render_away_preview(
    reason: AwayReason,
    custom_message: &str,
    duration: AwayDuration,
    language: Language,
) -> String {
    let template = match reason {
        AwayReason::Takeout => text::AWAY_TAKEOUT_MESSAGE.get(language),
        AwayReason::Restroom => text::AWAY_RESTROOM_MESSAGE.get(language),
        AwayReason::Custom => custom_message,
    };
    let seconds = duration.duration().as_secs();
    template.replace(
        "{{time}}",
        &format!("{:02}:{:02}", seconds / 60, seconds % 60),
    )
}

fn config_with_language(committed: &AppConfig, language: Language) -> AppConfig {
    committed.with_localized_defaults(language)
}

fn apply_language_managed_fields(target: &mut AppConfig, localized: &AppConfig) -> bool {
    let templates_changed = target.message_template != localized.message_template
        || target.round_report_template != localized.round_report_template
        || target.away_custom_message != localized.away_custom_message;
    target.language = localized.language;
    target.message_template = localized.message_template.clone();
    target.message_template_presets = localized.message_template_presets.clone();
    target.message_template_preset_names = localized.message_template_preset_names.clone();
    target.round_report_template = localized.round_report_template.clone();
    target.round_report_template_presets = localized.round_report_template_presets.clone();
    target.round_report_template_preset_names =
        localized.round_report_template_preset_names.clone();
    target.away_custom_message = localized.away_custom_message.clone();
    templates_changed
}

fn preview_snapshot_for_state(
    snapshot: &GameSnapshot,
    state: TemplatePreviewState,
) -> GameSnapshot {
    let mut preview = snapshot.clone();
    // Live Preview intentionally suppresses the WASD-idle branch. This copy is
    // preview-only; the runtime snapshot and the real OSC path are untouched.
    preview.no_wasd_for_10s = false;
    preview.round_report = None;

    match state {
        TemplatePreviewState::Normal => {
            preview.phase = RoundPhase::Combat;
            preview.round_metrics_active = true;
            // The preview is useful before VRChat produces any live data too.
            if !preview.has_damage_data {
                preview.has_damage_data = true;
                preview.latest_dps = 128;
                preview.average_dps = 96.4;
                preview.round_average_dps = 92.1;
                preview.round_effective_dps = 104.8;
                preview.round_burst_10s_dps = Some(146.2);
                preview.round_damage_taken = 24;
                preview.max_dps = 173;
                preview.has_max_dps_data = true;
            }
        }
        TemplatePreviewState::RoundReport => {
            // Template selection gives Combat precedence over round_report, so
            // a report preview must explicitly simulate the upgrade lobby.
            preview.phase = RoundPhase::Lobby;
            preview.round_metrics_active = false;
            preview.round_report = snapshot.round_report.clone().or(Some(RoundReport {
                has_duration_data: true,
                has_output_data: true,
                duration_seconds: 367,
                total_damage: 12_480,
                average_dps: 38.0,
                max_dps: 146,
                effective_dps: 82.4,
                burst_10s_dps: Some(126.7),
                dps_growth_rate: 18.4,
                has_dps_growth_rate: true,
                damage_taken: 73,
                has_longest_standstill_data: true,
                longest_standstill_seconds: 74,
            }));
            if !preview.has_step_estimate {
                preview.has_step_estimate = true;
                preview.current_step = 10;
                preview.until_boss_step = 1;
            }
        }
    }
    preview
}

impl eframe::App for AnalyzerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.runtime.shared.shutdown.load(Ordering::Relaxed) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        self.process_events(ctx);
        self.sync_overlay_position();
        let snapshot = self.runtime.shared.snapshot.read().clone();
        self.detect_self_lock_edge(&snapshot);
        self.settings_ui(ctx, &snapshot);
        self.overlay_ui(ctx, &snapshot);
        #[cfg(target_os = "windows")]
        ensure_windows_overlay_layering(self.draft.language);
        ctx.request_repaint_after(Duration::from_millis(100));
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        // Explicitly release the native clipboard before winit tears down its
        // platform event loop, as recommended by arboard for GUI frameworks.
        self.clipboard = None;
        self.runtime.shutdown();
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }
}

#[cfg(target_os = "windows")]
fn ensure_windows_overlay_layering(language: Language) {
    use windows_sys::Win32::{
        Foundation::COLORREF,
        UI::WindowsAndMessaging::{
            FindWindowW, GWL_EXSTYLE, GetLayeredWindowAttributes, GetWindowLongPtrW, LWA_ALPHA,
            LWA_COLORKEY, SetLayeredWindowAttributes, SetWindowLongPtrW, WS_EX_LAYERED,
        },
    };

    let title: Vec<u16> = overlay_window_title(language)
        .encode_utf16()
        .chain(Some(0))
        .collect();

    // SAFETY: the title is NUL-terminated, the HWND is checked before use, and
    // each call only changes documented top-level window composition flags.
    unsafe {
        let window = FindWindowW(std::ptr::null(), title.as_ptr());
        if window.is_null() {
            return;
        }
        let extended_style = GetWindowLongPtrW(window, GWL_EXSTYLE);
        if extended_style & WS_EX_LAYERED as isize == 0 {
            SetWindowLongPtrW(window, GWL_EXSTYLE, extended_style | WS_EX_LAYERED as isize);
        }

        let expected_flags = LWA_COLORKEY | LWA_ALPHA;
        let mut color_key: COLORREF = 0;
        let mut alpha = 0;
        let mut flags = 0;
        let attributes_intact =
            GetLayeredWindowAttributes(window, &mut color_key, &mut alpha, &mut flags) != 0
                && color_key == 0
                && alpha == 230
                && flags & expected_flags == expected_flags;

        if !attributes_intact {
            // Toggling click-through or window movement updates the HWND's
            // extended styles on Windows and can discard these attributes.
            // Glow/WGL may expose an opaque black backbuffer, so restore exact
            // black as the color key and softly blend the remaining card.
            SetLayeredWindowAttributes(window, 0, 230, expected_flags);
        }
    }
}

fn overlay_window_title(language: Language) -> String {
    format!(
        "{} v{APP_VERSION}",
        text::OVERLAY_WINDOW_TITLE.get(language)
    )
}

fn dps_chart_round_context(snapshot: &GameSnapshot, language: Language) -> Option<(u64, String)> {
    let active_epoch = (snapshot.phase == RoundPhase::Combat)
        .then_some(snapshot.combat_round_epoch)
        .filter(|epoch| *epoch > 0);
    let latest_epoch = snapshot
        .dps_history
        .iter()
        .rev()
        .find_map(|point| (point.combat_round_epoch > 0).then_some(point.combat_round_epoch));
    let selected_epoch = active_epoch.or(latest_epoch)?;
    let estimated_step = snapshot
        .dps_history
        .iter()
        .rev()
        .filter(|point| point.combat_round_epoch == selected_epoch)
        .find_map(|point| point.estimated_step)
        .or_else(|| snapshot.has_step_estimate.then_some(snapshot.current_step));
    let title = match (estimated_step, active_epoch == Some(selected_epoch)) {
        (Some(step), true) => format_pattern(
            text::CHART_CURRENT_ESTIMATED_ROUND,
            language,
            &[("step", step.to_string())],
        ),
        (Some(step), false) => format_pattern(
            text::CHART_FINISHED_ESTIMATED_ROUND,
            language,
            &[("step", step.to_string())],
        ),
        (None, true) => text::CHART_CURRENT_ROUND.get(language).to_owned(),
        (None, false) => text::CHART_FINISHED_ROUND.get(language).to_owned(),
    };
    Some((selected_epoch, title))
}

fn dps_history_chart(
    ui: &mut egui::Ui,
    snapshot: &GameSnapshot,
    view: &mut DpsChartViewState,
    language: Language,
) {
    if snapshot.dps_history.is_empty() {
        let text = if snapshot.in_ecliptica {
            text::CHART_WAITING_FIRST_SECOND.get(language)
        } else {
            text::CHART_ENTER_ECLIPTICA.get(language)
        };
        Empty::show(ui, |ui| {
            Typography::new(text)
                .strong()
                .color(SETTINGS_HEADING)
                .show(ui);
        });
        return;
    }

    let Some((selected_epoch, round_title)) = dps_chart_round_context(snapshot, language) else {
        Empty::show(ui, |ui| {
            Typography::new(text::CHART_WAITING_ROUND.get(language))
                .strong()
                .color(SETTINGS_HEADING)
                .show(ui);
        });
        return;
    };
    if Some(selected_epoch) != view.selected_epoch {
        view.selected_epoch = Some(selected_epoch);
    }

    let selected_points = snapshot
        .dps_history
        .iter()
        .filter(|point| point.combat_round_epoch == selected_epoch)
        .collect::<Vec<_>>();
    if selected_points.is_empty() {
        Empty::show(ui, |ui| {
            Typography::new(text::CHART_WAITING_DATA.get(language))
                .strong()
                .color(SETTINGS_HEADING)
                .show(ui);
        });
        return;
    }

    let estimated_step = selected_points
        .iter()
        .rev()
        .find_map(|point| point.estimated_step)
        .or_else(|| snapshot.has_step_estimate.then_some(snapshot.current_step));

    let raw = selected_points
        .iter()
        .map(|point| [point.elapsed_seconds as f64, point.dps as f64])
        .collect::<Vec<_>>();
    let trend = dps_trend_points(&raw);
    let reduced_trend = downsample_dps_trend(&trend, DPS_CHART_MAX_TREND_POINTS);
    let smooth_trend = smooth_chart_points(&reduced_trend, 4);
    let raw_peak = chart_peak(&raw).unwrap_or(raw[0]);
    // Bars need a meaningful zero baseline. The raw peak remains part of the
    // auto-fit even though the foreground line intentionally shows a trend.
    let mut view_points = raw.clone();
    view_points.push([raw.last().map_or(0.0, |point| point[0]), 0.0]);
    let best_view = chart_best_view_bounds(&view_points);
    let now = Instant::now();
    // The previous frame's expanded rect includes the x/y axis widgets. Record
    // pointer presence before Plot::show so an auto-fit due on the same frame
    // cannot overwrite an axis drag or resize.
    if view.pointer_is_in_chart(ui) {
        view.record_user_interaction(now);
    }
    let auto_fit_due = view.should_auto_fit(now);
    let bars = BarChart::new(
        text::DPS_PER_SECOND.get(language),
        raw.iter()
            .filter(|point| point[1] > 0.0)
            .map(|point| {
                Bar::new(point[0], point[1])
                    .width(0.72)
                    .fill(SETTINGS_ACCENT.gamma_multiply(0.24))
                    .stroke(egui::Stroke::new(0.8, SETTINGS_ACCENT.gamma_multiply(0.42)))
            })
            .collect(),
    )
    .allow_hover(false);
    let glow = Line::new(
        text::DPS_AVERAGE.get(language),
        PlotPoints::new(smooth_trend.clone()),
    )
    .color(egui::Color32::from_rgba_unmultiplied(151, 122, 255, 70))
    .width(6.0)
    .allow_hover(false);
    let line = Line::new(
        text::DPS_AVERAGE.get(language),
        PlotPoints::new(smooth_trend.clone()),
    )
    .color(SETTINGS_CHART_LINE)
    .width(2.4)
    .allow_hover(false);
    let accessibility_summary = format_pattern(
        text::DPS_CHART_ACCESSIBILITY,
        language,
        &[
            ("round", round_title),
            ("peak", format!("{:.0}", raw_peak[1])),
            ("time", format_chart_elapsed(raw_peak[0], language)),
        ],
    );
    let round_markers = chart_round_markers(
        &snapshot.dps_history,
        estimated_step.map(|step| (selected_epoch, step)),
    );

    egui::Frame::NONE
        .fill(SETTINGS_CHART_BG)
        .corner_radius(8.0)
        .stroke(egui::Stroke::new(1.0, SETTINGS_INSET_BORDER))
        .inner_margin(egui::Margin::same(UI_SPACE_2 as i8))
        .show(ui, |ui| {
            ui.scope(|ui| {
                let visuals = &mut ui.style_mut().visuals;
                visuals.override_text_color = Some(SETTINGS_CHART_AXIS);
                visuals.weak_text_color = Some(SETTINGS_CHART_AXIS);
                visuals.weak_text_alpha = 1.0;
                visuals.extreme_bg_color = SETTINGS_CHART_BG;
                visuals.widgets.noninteractive.bg_stroke = egui::Stroke::NONE;
                visuals.widgets.noninteractive.fg_stroke =
                    egui::Stroke::new(1.0, SETTINGS_CHART_AXIS);

                let x_axis_width = ui.available_width();
                let x_axis_title_center_x = ui.available_rect_before_wrap().center().x;
                let response = Plot::new("overview-dps-history")
                    .height(250.0)
                    .show_background(false)
                    .show_grid([false, false])
                    .allow_drag([true, true])
                    .allow_zoom([true, true])
                    .allow_axis_zoom_drag([true, true])
                    // Leave ordinary wheel scrolling to the outer settings
                    // page. Axis ranges are adjusted through axis dragging or
                    // explicit zoom gestures, never by scrolling the chart.
                    .allow_scroll(false)
                    .allow_boxed_zoom(false)
                    .set_margin_fraction(egui::vec2(0.035, 0.0))
                    .y_axis_label("DPS")
                    .x_axis_formatter(move |mark, range| {
                        format_chart_x_tick_localized(
                            mark.value,
                            range,
                            &round_markers,
                            language,
                            x_axis_width,
                        )
                    })
                    .y_axis_formatter(move |mark, range| {
                        format_chart_y_tick(mark.value, mark.step_size, range, language)
                    })
                    .show_x(false)
                    .show_y(false)
                    .cursor_color(SETTINGS_CHART_CURSOR)
                    .show(ui, |plot_ui| {
                        let zooming = plot_ui.response().contains_pointer()
                            && plot_ui
                                .ctx()
                                .input(|input| input.zoom_delta_2d() != egui::Vec2::splat(1.0));
                        let user_interacting =
                            plot_ui.response().hovered() || plot_ui.response().dragged() || zooming;
                        let peak_screen_position = plot_ui
                            .screen_from_plot(egui_plot::PlotPoint::new(raw_peak[0], raw_peak[1]));
                        // Give the small peak marker a 24-point hit target so it
                        // remains easy to acquire at dense or zoomed-out scales.
                        let peak_hovered = plot_ui.response().hover_pos().is_some_and(|pointer| {
                            chart_peak_is_hovered(pointer, peak_screen_position)
                        });
                        if auto_fit_due && !user_interacting {
                            plot_ui.set_plot_bounds_x(best_view.0.0..=best_view.0.1);
                            plot_ui.set_plot_bounds_y(best_view.1.0..=best_view.1.1);
                        }
                        plot_ui.bar_chart(bars);
                        plot_ui.line(glow);
                        plot_ui.line(line);
                        plot_ui.points(
                            Points::new(text::DPS_ROUND_PEAK.get(language), vec![raw_peak])
                                .shape(MarkerShape::Diamond)
                                .color(SETTINGS_WARNING)
                                .radius(if peak_hovered { 8.0 } else { 5.5 })
                                .filled(true)
                                .allow_hover(false),
                        );
                        if peak_hovered {
                            plot_ui.points(
                                Points::new("", vec![raw_peak])
                                    .shape(MarkerShape::Diamond)
                                    .color(SETTINGS_CHART_AXIS)
                                    .radius(10.0)
                                    .filled(false)
                                    .allow_hover(false),
                            );
                        }
                        if smooth_trend.len() == 1 {
                            plot_ui.points(
                                Points::new("", smooth_trend.clone())
                                    .color(SETTINGS_CHART_LINE)
                                    .radius(4.5)
                                    .allow_hover(false),
                            );
                        }
                        if plot_ui.response().hovered() && !plot_ui.response().dragged() {
                            let hovered_raw_point = if peak_hovered {
                                Some(raw_peak)
                            } else {
                                plot_ui
                                    .pointer_coordinate()
                                    .and_then(|pointer| chart_nearest_point_at_x(&raw, pointer.x))
                            };
                            if let Some(raw_point) = hovered_raw_point {
                                let trend_point =
                                    chart_point_at_x(&trend, raw_point[0]).unwrap_or(raw_point);
                                plot_ui.vline(
                                    VLine::new("", raw_point[0])
                                        .color(SETTINGS_CHART_CURSOR)
                                        .width(1.0)
                                        .allow_hover(false),
                                );
                                plot_ui.hline(
                                    HLine::new("", raw_point[1])
                                        .color(SETTINGS_CHART_CURSOR)
                                        .width(1.0)
                                        .allow_hover(false),
                                );
                                plot_ui.points(
                                    Points::new("", vec![raw_point])
                                        .shape(if peak_hovered {
                                            MarkerShape::Diamond
                                        } else {
                                            MarkerShape::Square
                                        })
                                        .color(if peak_hovered {
                                            SETTINGS_WARNING
                                        } else {
                                            SETTINGS_ACCENT
                                        })
                                        .radius(if peak_hovered { 8.0 } else { 4.0 })
                                        .allow_hover(false),
                                );
                                plot_ui.points(
                                    Points::new("", vec![trend_point])
                                        .shape(MarkerShape::Circle)
                                        .color(SETTINGS_CHART_AXIS)
                                        .radius(3.5)
                                        .allow_hover(false),
                                );
                            }
                        }
                        (user_interacting, peak_hovered)
                    });
                if response.inner.1 {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
                }
                response.response.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::Image,
                        ui.is_enabled(),
                        accessibility_summary.clone(),
                    )
                });
                if response.response.hovered() && !response.response.dragged() {
                    let hovered_sample = response.response.hover_pos().and_then(|pointer_pos| {
                        let raw_point = if response.inner.1 {
                            raw_peak
                        } else {
                            let pointer = response.transform.value_from_position(pointer_pos);
                            chart_nearest_point_at_x(&raw, pointer.x)?
                        };
                        let trend_point = chart_point_at_x(&trend, raw_point[0])?;
                        Some((raw_point, trend_point))
                    });
                    if let Some((raw_point, trend_point)) = hovered_sample {
                        paint_chart_tooltip(
                            ui,
                            &response.response,
                            format_pattern(
                                if response.inner.1 {
                                    text::DPS_CHART_PEAK_TOOLTIP
                                } else {
                                    text::DPS_CHART_TOOLTIP
                                },
                                language,
                                &[
                                    ("time", format_chart_elapsed(raw_point[0], language)),
                                    ("raw", format!("{:.0}", raw_point[1])),
                                    ("trend", format!("{:.1}", trend_point[1])),
                                ],
                            ),
                        );
                    }
                }
                // Axis widgets live just outside the central plot response, so
                // include their surrounding bands when detecting manual
                // range changes.
                let complete_chart_rect = response.response.rect.expand2(egui::vec2(64.0, 48.0));
                view.interaction_rect = Some(complete_chart_rect);
                let axis_or_plot_dragging = ui.input(|input| {
                    input.pointer.is_decidedly_dragging()
                        && input
                            .pointer
                            .interact_pos()
                            .is_some_and(|position| complete_chart_rect.contains(position))
                });
                if response.response.hovered() || response.inner.0 || axis_or_plot_dragging {
                    view.record_user_interaction(now);
                } else if auto_fit_due {
                    view.record_auto_fit(now);
                }
                let next_auto_fit = view.next_auto_fit_in(now);
                if !next_auto_fit.is_zero() {
                    ui.ctx().request_repaint_after(next_auto_fit);
                }
                ui.add_space(DPS_CHART_X_AXIS_TITLE_GAP);
                let title_galley = ui.painter().layout_no_wrap(
                    text::SESSION_TIME.get(language).to_owned(),
                    egui::TextStyle::Body.resolve(ui.style()),
                    SETTINGS_CHART_AXIS,
                );
                let (_, title_rect) =
                    ui.allocate_space(egui::vec2(ui.available_width(), title_galley.size().y));
                ui.painter().galley(
                    egui::pos2(
                        x_axis_title_center_x - title_galley.size().x / 2.0,
                        title_rect.top(),
                    ),
                    title_galley,
                    SETTINGS_CHART_AXIS,
                );
            });
        });
}

fn paint_chart_tooltip(ui: &egui::Ui, response: &egui::Response, text: String) {
    let Some(pointer) = response.hover_pos() else {
        return;
    };
    let painter = ui.painter();
    let font_id = egui::TextStyle::Body.resolve(ui.style());
    let padding = egui::vec2(9.0, 6.0);
    let tooltip_width = (response.rect.width() - padding.x * 2.0 - 8.0).clamp(40.0, 260.0);
    let galley = painter.layout(text, font_id, SETTINGS_CHART_AXIS, tooltip_width);
    let size = galley.size() + padding * 2.0;
    let mut position = pointer + egui::vec2(14.0, 14.0);
    let safe_rect = response.rect.shrink(4.0);
    if position.x + size.x > safe_rect.right() {
        position.x = pointer.x - size.x - 14.0;
    }
    if position.y + size.y > safe_rect.bottom() {
        position.y = pointer.y - size.y - 14.0;
    }
    position.x = position
        .x
        .clamp(safe_rect.left(), safe_rect.right() - size.x);
    position.y = position
        .y
        .clamp(safe_rect.top(), safe_rect.bottom() - size.y);
    let rect = egui::Rect::from_min_size(position, size);
    painter.rect(
        rect,
        6.0,
        egui::Color32::from_rgb(46, 39, 61),
        egui::Stroke::new(1.0, SETTINGS_PREVIEW_BORDER),
        egui::StrokeKind::Inside,
    );
    painter.galley(position + padding, galley, SETTINGS_CHART_AXIS);
}

fn chart_point_at_x(points: &[[f64; 2]], x: f64) -> Option<[f64; 2]> {
    let first = *points.first()?;
    let last = *points.last()?;
    if x < first[0] || x > last[0] {
        return None;
    }
    match points.binary_search_by(|point| point[0].total_cmp(&x)) {
        Ok(index) => Some(points[index]),
        Err(upper) if upper > 0 && upper < points.len() => {
            let left = points[upper - 1];
            let right = points[upper];
            let span = right[0] - left[0];
            let ratio = if span.abs() > f64::EPSILON {
                (x - left[0]) / span
            } else {
                0.0
            };
            Some([x, left[1] + (right[1] - left[1]) * ratio])
        }
        _ => None,
    }
}

fn chart_nearest_point_at_x(points: &[[f64; 2]], x: f64) -> Option<[f64; 2]> {
    let first = *points.first()?;
    let last = *points.last()?;
    if x < first[0] - 0.5 || x > last[0] + 0.5 {
        return None;
    }
    let x = x.clamp(first[0], last[0]);
    match points.binary_search_by(|point| point[0].total_cmp(&x)) {
        Ok(index) => Some(points[index]),
        Err(upper) if upper > 0 && upper < points.len() => {
            let left = points[upper - 1];
            let right = points[upper];
            if x - left[0] <= right[0] - x {
                Some(left)
            } else {
                Some(right)
            }
        }
        _ => None,
    }
}

fn chart_peak_is_hovered(pointer: egui::Pos2, peak: egui::Pos2) -> bool {
    pointer.distance(peak) <= DPS_CHART_PEAK_HIT_RADIUS
}

#[cfg(test)]
fn format_chart_x_tick(
    seconds: f64,
    visible_range: &std::ops::RangeInclusive<f64>,
    round_markers: &[ChartRoundMarker],
) -> String {
    format_chart_x_tick_localized(
        seconds,
        visible_range,
        round_markers,
        Language::Chinese,
        600.0,
    )
}

fn format_chart_x_tick_localized(
    seconds: f64,
    visible_range: &std::ops::RangeInclusive<f64>,
    round_markers: &[ChartRoundMarker],
    language: Language,
    axis_width: f32,
) -> String {
    let span = (*visible_range.end() - *visible_range.start()).abs();
    let is_origin = seconds.abs() < 0.5;
    let elapsed = format_chart_elapsed(seconds, language);
    let label = chart_round_at(seconds, round_markers).map_or(elapsed.clone(), |step| {
        format!(
            "{elapsed} · {}",
            format_pattern(text::ROUND_TICK, language, &[("step", step.to_string())])
        )
    });
    // egui_plot centers each label on its tick without clipping it to the
    // axis. Convert half of this label's estimated rendered width back into
    // axis units so long elapsed-time/round labels near the right edge are
    // omitted before they can spill outside the chart frame.
    let estimated_label_width = label
        .chars()
        .map(|character| if character.is_ascii() { 7.0 } else { 14.0 })
        .sum::<f64>();
    let edge_guard = span * (estimated_label_width / 2.0 + 6.0) / f64::from(axis_width.max(1.0));
    if !is_origin
        && (seconds - *visible_range.start() < edge_guard
            || *visible_range.end() - seconds < edge_guard)
    {
        return String::new();
    }
    label
}

fn chart_round_markers(
    history: &[DpsHistoryPoint],
    current_anchor: Option<(u64, u32)>,
) -> Vec<ChartRoundMarker> {
    let anchor = current_anchor.or_else(|| {
        history.iter().rev().find_map(|point| {
            point
                .estimated_step
                .map(|step| (point.combat_round_epoch, step))
        })
    });
    let Some((anchor_epoch, anchor_step)) = anchor else {
        return Vec::new();
    };

    let mut previous_epoch = 0;
    history
        .iter()
        .filter_map(|point| {
            let epoch = point.combat_round_epoch;
            if epoch == 0 || epoch == previous_epoch {
                return None;
            }
            previous_epoch = epoch;
            let relative_step =
                i128::from(anchor_step) + i128::from(epoch) - i128::from(anchor_epoch);
            let step = u32::try_from(relative_step).ok().filter(|step| *step > 0)?;
            Some(ChartRoundMarker {
                start_seconds: point.elapsed_seconds as f64,
                step,
            })
        })
        .collect()
}

fn chart_round_at(seconds: f64, markers: &[ChartRoundMarker]) -> Option<u32> {
    markers
        .partition_point(|marker| marker.start_seconds <= seconds)
        .checked_sub(1)
        .map(|index| markers[index].step)
}

fn chart_best_view_bounds(points: &[[f64; 2]]) -> ((f64, f64), (f64, f64)) {
    let data_x_min = points
        .iter()
        .filter_map(|[x, y]| (x.is_finite() && y.is_finite()).then_some(*x))
        .fold(f64::INFINITY, f64::min);
    let x_max = points
        .iter()
        .filter_map(|[x, y]| (x.is_finite() && y.is_finite()).then_some(*x))
        .fold(f64::NEG_INFINITY, f64::max);
    if !data_x_min.is_finite() || !x_max.is_finite() {
        return ((0.0, 10.0), (0.0, 1.0));
    }

    let recent_start = (x_max - DPS_CHART_RECENT_WINDOW_SECONDS).max(data_x_min);
    let mut x_max = f64::NEG_INFINITY;
    let mut y_min = f64::INFINITY;
    let mut y_max = f64::NEG_INFINITY;
    for [x, y] in points.iter().copied() {
        if x.is_finite() && y.is_finite() && x >= recent_start {
            x_max = x_max.max(x);
            y_min = y_min.min(y);
            y_max = y_max.max(y);
        }
    }

    let x_span = x_max - recent_start;
    let x_padding = if x_span > f64::EPSILON {
        (x_span * DPS_CHART_X_MARGIN_FRACTION).max(1.0)
    } else {
        5.0
    };
    let x_bounds = ((recent_start - x_padding).max(0.0), x_max + x_padding);

    let y_span = y_max - y_min;
    let y_padding = if y_span > f64::EPSILON {
        (y_span * DPS_CHART_Y_MARGIN_FRACTION).max(1.0)
    } else {
        (y_max.abs() * DPS_CHART_Y_MARGIN_FRACTION).max(1.0)
    };
    let mut y_bounds = ((y_min - y_padding).max(0.0), y_max + y_padding);
    if y_bounds.1 - y_bounds.0 < DPS_CHART_MIN_Y_SPAN {
        let center = (y_bounds.0 + y_bounds.1) / 2.0;
        y_bounds = (
            center - DPS_CHART_MIN_Y_SPAN / 2.0,
            center + DPS_CHART_MIN_Y_SPAN / 2.0,
        );
        if y_bounds.0 < 0.0 {
            y_bounds.1 -= y_bounds.0;
            y_bounds.0 = 0.0;
        }
    }

    (x_bounds, y_bounds)
}

fn format_chart_y_tick(
    value: f64,
    step_size: f64,
    visible_range: &std::ops::RangeInclusive<f64>,
    language: Language,
) -> String {
    let span = (*visible_range.end() - *visible_range.start()).abs();
    if span >= 100.0 && step_size >= 1.0 {
        return format_compact_number(value, language);
    }

    let decimals = if step_size.is_finite() && step_size > 0.0 && step_size < 1.0 {
        ((-step_size.log10()).ceil() as usize + 1).clamp(1, 4)
    } else {
        0
    };
    let formatted = format!("{value:.decimals$}");
    if decimals == 0 {
        formatted
    } else {
        formatted
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_owned()
    }
}

fn dps_trend_points(points: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let mut trend = 0.0;
    points
        .iter()
        .map(|point| {
            // Restore the original asymmetric smoothing: attacks remain
            // responsive while releases decay slowly enough to show the
            // sustained combat shape instead of every zero-to-hit transition.
            let alpha = if point[1] >= trend { 0.28 } else { 0.12 };
            trend += (point[1] - trend) * alpha;
            if trend < 0.05 {
                trend = 0.0;
            }
            [point[0], trend]
        })
        .collect()
}

fn chart_peak(points: &[[f64; 2]]) -> Option<[f64; 2]> {
    points
        .iter()
        .copied()
        .reduce(|peak, point| if point[1] > peak[1] { point } else { peak })
}

fn downsample_dps_trend(points: &[[f64; 2]], max_points: usize) -> Vec<[f64; 2]> {
    if points.len() <= max_points || max_points < 3 {
        return points.to_vec();
    }
    let interior_slots = max_points - 2;
    let bucket_size = (points.len() - 2).div_ceil(interior_slots);
    let mut reduced = Vec::with_capacity(max_points);
    reduced.push(points[0]);
    for bucket in points[1..points.len() - 1].chunks(bucket_size) {
        let divisor = bucket.len() as f64;
        reduced.push([
            bucket.iter().map(|point| point[0]).sum::<f64>() / divisor,
            bucket.iter().map(|point| point[1]).sum::<f64>() / divisor,
        ]);
    }
    reduced.push(points[points.len() - 1]);
    reduced
}

fn smooth_chart_points(points: &[[f64; 2]], subdivisions: usize) -> Vec<[f64; 2]> {
    if points.len() < 2 || subdivisions == 0 {
        return points.to_vec();
    }
    let mut output = Vec::with_capacity((points.len() - 1) * subdivisions + 1);
    for index in 0..points.len() - 1 {
        let p1 = points[index];
        let p2 = points[index + 1];
        for step in 0..subdivisions {
            let t = step as f64 / subdivisions as f64;
            let eased = t * t * (3.0 - 2.0 * t);
            output.push([p1[0] + (p2[0] - p1[0]) * t, p1[1] + (p2[1] - p1[1]) * eased]);
        }
    }
    output.push(points[points.len() - 1]);
    output
}

fn format_chart_elapsed(seconds: f64, language: Language) -> String {
    let seconds = seconds.max(0.0).round() as u64;
    let hours = seconds / 3_600;
    let minutes = seconds % 3_600 / 60;
    let seconds = seconds % 60;
    if hours > 0 {
        if minutes > 0 {
            format_pattern(
                text::ELAPSED_HOURS_MINUTES,
                language,
                &[
                    ("hours", hours.to_string()),
                    ("minutes", format!("{minutes:02}")),
                ],
            )
        } else {
            format_pattern(
                text::ELAPSED_HOURS,
                language,
                &[("hours", hours.to_string())],
            )
        }
    } else if minutes > 0 {
        if seconds > 0 {
            format_pattern(
                text::ELAPSED_MINUTES_SECONDS,
                language,
                &[
                    ("minutes", minutes.to_string()),
                    ("seconds", format!("{seconds:02}")),
                ],
            )
        } else {
            format_pattern(
                text::ELAPSED_MINUTES,
                language,
                &[("minutes", minutes.to_string())],
            )
        }
    } else {
        format_pattern(
            text::ELAPSED_SECONDS,
            language,
            &[("seconds", seconds.to_string())],
        )
    }
}

fn localized_standstill(report: &RoundReport, language: Language) -> String {
    if report.has_longest_standstill_data {
        format_pattern(
            text::SECONDS_VALUE,
            language,
            &[("seconds", report.longest_standstill_seconds.to_string())],
        )
    } else {
        "-".to_owned()
    }
}

fn format_compact_number(value: f64, language: Language) -> String {
    compact_metric(value, false, language)
}

fn sidebar_notice(ui: &mut egui::Ui, message: &str, tone: SidebarNoticeTone) -> egui::Response {
    let color = match tone {
        SidebarNoticeTone::Success => SETTINGS_SUCCESS,
        SidebarNoticeTone::Warning => SETTINGS_WARNING,
        SidebarNoticeTone::Error => SETTINGS_DANGER,
    };
    let job = sidebar_notice_layout_job(message, color, ui.available_width());
    let galley = ui.painter().layout_job(job);
    ui.add(egui::Label::new(galley).wrap())
        .on_hover_text(message)
}

fn sidebar_notice_height(ui: &egui::Ui, message: &str, width: f32) -> f32 {
    let font_id = egui::FontId::proportional(13.0);
    let line_count: f32 = message
        .split('\n')
        .map(|line| {
            let measured_width = ui
                .painter()
                .layout_no_wrap(line.to_owned(), font_id.clone(), egui::Color32::WHITE)
                .size()
                .x;
            // Headless test contexts can report zero before their font atlas is
            // populated. This conservative fallback also keeps unusual missing
            // glyphs from making the footer too short in the real window.
            let fallback_width: f32 = line
                .chars()
                .map(|character| if character.is_ascii() { 7.0 } else { 13.0 })
                .sum();
            let text_width = measured_width.max(fallback_width);
            (text_width / width.max(1.0)).ceil().max(1.0)
        })
        .sum();
    line_count * 18.0
}

fn sidebar_notice_layout_job(
    message: &str,
    color: egui::Color32,
    width: f32,
) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    job.wrap.max_width = width;
    job.wrap.break_anywhere = true;
    job.append(
        message,
        0.0,
        egui::TextFormat {
            font_id: egui::FontId::proportional(13.0),
            color,
            line_height: Some(18.0),
            ..Default::default()
        },
    );
    job
}

fn nav_button(
    ui: &mut egui::Ui,
    page: &mut SettingsPage,
    target: SettingsPage,
    label: &str,
    icon: LucideIcon,
) {
    let selected = *page == target;
    ui.set_width(ui.available_width());
    let response = ui
        .with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), |ui| {
            ShadcnButton::new(label)
                .icon(icon)
                .variant(if selected {
                    ButtonVariant::Secondary
                } else {
                    ButtonVariant::Ghost
                })
                .selected(selected)
                .full_width()
                .height(40.0)
                .horizontal_padding(12.0)
                .corner_radius(6.0)
                .show(ui)
        })
        .inner;
    if response.clicked() {
        *page = target;
    }
    ui.add_space(UI_SPACE_1);
}

struct VariableHelp<'a> {
    role: &'a str,
    name: &'a str,
    description: &'a str,
    enabled: bool,
}

struct VariableHelpGroup<'a> {
    title: &'a str,
    color: egui::Color32,
    variables: Vec<VariableHelp<'a>>,
}

const VARIABLE_GROUP_LABEL_WIDTH: f32 = 142.0;
const VARIABLE_GROUP_COLUMN_GAP: f32 = UI_SPACE_3;

#[cfg_attr(not(test), allow(dead_code))]
struct VariableGroupRowLayout {
    label_rect: egui::Rect,
    label_text_rect: egui::Rect,
    buttons_start_x: f32,
    button_rects: Vec<egui::Rect>,
}

fn variable_group_color(group: &ecliptica_data_analyzer::i18n::VariableCopyGroup) -> egui::Color32 {
    match group.variables.first().map(|variable| variable.name) {
        Some("latest_dps") => METRIC_LIVE_DPS,
        Some("avg_dps") => METRIC_AVERAGE_DPS,
        Some("round_effective_dps" | "round_report_effective_dps") => METRIC_ACTIVE_DPS,
        Some("round_burst_10s" | "round_report_burst_10s") => VARIABLE_BEST_AVERAGE_DPS,
        Some("round_damage_taken") => METRIC_DAMAGE_TAKEN,
        Some("max_dps" | "round_max_dps") => VARIABLE_HIGHEST_DPS,
        Some("boss_lock") => VARIABLE_COMBAT,
        Some("is_self_boss_locked") => VARIABLE_ALERT,
        Some("has_step_estimate") => VARIABLE_GAME_PROGRESS,
        Some("round_duration") => VARIABLE_DURATION,
        Some("dps_growth_rate") => VARIABLE_DPS_GROWTH,
        Some("round_longest_standstill") => VARIABLE_STANDSTILL,
        Some("round_total_damage") => VARIABLE_ROUND_DAMAGE,
        Some("heart_rate") => VARIABLE_HEART_RATE,
        _ => SETTINGS_TEXT_SECONDARY,
    }
}

fn variable_chip_color(group_color: egui::Color32, name: &str) -> egui::Color32 {
    if !name.starts_with("has_") {
        return group_color;
    }

    // Availability flags remain in their category's hue, but sit one step
    // darker than the value they guard.
    const FLAG_SHADE: f32 = 0.78;
    egui::Color32::from_rgb(
        (f32::from(group_color.r()) * FLAG_SHADE).round() as u8,
        (f32::from(group_color.g()) * FLAG_SHADE).round() as u8,
        (f32::from(group_color.b()) * FLAG_SHADE).round() as u8,
    )
}

fn variable_chip(
    ui: &mut egui::Ui,
    variable: &VariableHelp<'_>,
    group_color: egui::Color32,
    clipboard: &mut Option<Clipboard>,
    toast_state: &mut ToastState,
    language: Language,
) -> egui::Response {
    let name = variable.name;
    let color = variable_chip_color(group_color, name);
    let token = format!("{{{{{name}}}}}");
    let role = match variable.role {
        role if role == text::ROLE_CONDITION.get(language) => text::ROLE_JUDGMENT.get(language),
        _ => text::ROLE_DISPLAY.get(language),
    };
    let label = format!("{role} · {token}");
    let galley =
        ui.painter()
            .layout_no_wrap(label.clone(), egui::FontId::monospace(11.0), SETTINGS_TEXT);
    let icon_size = 11.0;
    let icon_gap = 4.0;
    let horizontal_padding = 6.0;
    let desired_size = egui::vec2(
        horizontal_padding * 2.0 + icon_size + icon_gap + galley.size().x,
        23.0,
    );
    let (rect, response) = ui.allocate_exact_size(
        desired_size,
        if variable.enabled {
            egui::Sense::click()
        } else {
            egui::Sense::hover()
        },
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label.clone())
    });

    if ui.is_rect_visible(rect) {
        let tint = |base: u8, accent: u8, hovered: bool| {
            if hovered {
                ((u16::from(base) * 2 + u16::from(accent)) / 3) as u8
            } else {
                ((u16::from(base) * 3 + u16::from(accent)) / 4) as u8
            }
        };
        let fill = if variable.enabled {
            egui::Color32::from_rgb(
                tint(SETTINGS_SURFACE.r(), color.r(), response.hovered()),
                tint(SETTINGS_SURFACE.g(), color.g(), response.hovered()),
                tint(SETTINGS_SURFACE.b(), color.b(), response.hovered()),
            )
        } else {
            SETTINGS_SURFACE
        };
        let foreground = if variable.enabled {
            SETTINGS_TEXT
        } else {
            SETTINGS_TEXT_MUTED
        };
        let painter = ui.painter();
        let corner = egui::CornerRadius::same(6);
        painter.rect_filled(rect, corner, fill);
        painter.rect_stroke(
            rect,
            corner,
            egui::Stroke::new(1.0, SETTINGS_BORDER),
            egui::epaint::StrokeKind::Inside,
        );
        let icon_rect = egui::Rect::from_min_size(
            egui::pos2(
                rect.left() + horizontal_padding,
                rect.center().y - icon_size / 2.0,
            ),
            egui::vec2(icon_size, icon_size),
        );
        egui_shadcn::paint_icon(painter, icon_rect, &LucideIcon::Copy, foreground);
        let text_pos = egui::pos2(
            icon_rect.right() + icon_gap,
            rect.center().y - galley.size().y / 2.0,
        );
        painter.galley(text_pos, galley, foreground);
        if response.has_focus() {
            egui_shadcn::paint::paint_focus_ring::paint_focus_ring(
                painter,
                rect,
                6.0,
                SETTINGS_ACCENT,
            );
        }
    }

    let response = if variable.enabled {
        response
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(format!(
                "{}\n{}",
                variable.description,
                format_pattern(
                    text::COPY_VARIABLE_HINT,
                    language,
                    &[("token", token.clone())]
                )
            ))
    } else {
        response.on_hover_text(format!(
            "{}\n{}",
            variable.description,
            text::HEART_RATE_VARIABLE_OFFLINE.get(language)
        ))
    };
    if !response.clicked() {
        return response;
    }

    let result = (|| -> Result<(), String> {
        if clipboard.is_none() {
            *clipboard = Some(Clipboard::new().map_err(|error| error.to_string())?);
        }
        clipboard
            .as_mut()
            .ok_or_else(|| text::CLIPBOARD_UNAVAILABLE.get(language).to_owned())?
            .set_text(token.clone())
            .map_err(|error| error.to_string())
    })();
    let now = ui.ctx().input(|input| input.time);
    match result {
        Ok(()) => toast_state.add(
            text::VARIABLE_COPIED.get(language),
            ToastVariant::Success,
            now,
        ),
        Err(error) => {
            // A later click can retry initialization after a transient OS lock.
            *clipboard = None;
            tracing::warn!(error, "复制模板变量失败");
            toast_state.add(text::COPY_FAILED.get(language), ToastVariant::Error, now);
        }
    }
    response
}

fn variable_help_group_row(
    ui: &mut egui::Ui,
    index: usize,
    group: &VariableHelpGroup<'_>,
    clipboard: &mut Option<Clipboard>,
    toast_state: &mut ToastState,
    language: Language,
) -> VariableGroupRowLayout {
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = VARIABLE_GROUP_COLUMN_GAP;
        let label_area = ui.allocate_ui_with_layout(
            egui::vec2(VARIABLE_GROUP_LABEL_WIDTH, 23.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.set_min_width(VARIABLE_GROUP_LABEL_WIDTH);
                ui.set_max_width(VARIABLE_GROUP_LABEL_WIDTH);
                Typography::small(format!("{:02} · {}", index + 1, group.title))
                    .strong()
                    .color(SETTINGS_TEXT_SECONDARY)
                    .truncate()
                    .show(ui)
            },
        );
        let label_rect = label_area.response.rect;
        let label_text_rect = label_area.inner.rect;

        let mut buttons_start_x = 0.0;
        let mut button_rects = Vec::with_capacity(group.variables.len());
        ui.vertical(|ui| {
            ui.set_width(ui.available_width());
            buttons_start_x = ui.cursor().left();
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(UI_SPACE_2, UI_SPACE_2);
                for variable in &group.variables {
                    button_rects.push(
                        variable_chip(ui, variable, group.color, clipboard, toast_state, language)
                            .rect,
                    );
                }
            });
        });

        VariableGroupRowLayout {
            label_rect,
            label_text_rect,
            buttons_start_x,
            button_rects,
        }
    })
    .inner
}

fn variable_help_groups(
    ui: &mut egui::Ui,
    groups: &[VariableHelpGroup<'_>],
    clipboard: &mut Option<Clipboard>,
    toast_state: &mut ToastState,
    language: Language,
) {
    for (index, group) in groups.iter().enumerate() {
        variable_help_group_row(ui, index, group, clipboard, toast_state, language);
        ui.add_space(UI_SPACE_2);
    }
}

fn localized_variable_groups(
    source: &'static [ecliptica_data_analyzer::i18n::VariableCopyGroup],
    language: Language,
    has_heart_rate: bool,
) -> Vec<VariableHelpGroup<'static>> {
    source
        .iter()
        .map(|group| {
            let variables = group
                .variables
                .iter()
                .map(|variable| VariableHelp {
                    role: variable.role.get(language),
                    name: variable.name,
                    description: variable.description.get(language),
                    enabled: !matches!(variable.name, "heart_rate" | "has_heart_rate")
                        || has_heart_rate,
                })
                .collect::<Vec<_>>();
            VariableHelpGroup {
                title: group.title.get(language),
                color: variable_group_color(group),
                variables,
            }
        })
        .collect()
}

fn live_variable_help(
    ui: &mut egui::Ui,
    clipboard: &mut Option<Clipboard>,
    toast_state: &mut ToastState,
    language: Language,
    has_heart_rate: bool,
) {
    let groups = localized_variable_groups(
        ecliptica_data_analyzer::i18n::LIVE_VARIABLE_GROUPS,
        language,
        has_heart_rate,
    );
    variable_help_groups(ui, &groups, clipboard, toast_state, language);
}

fn report_variable_help(
    ui: &mut egui::Ui,
    clipboard: &mut Option<Clipboard>,
    toast_state: &mut ToastState,
    language: Language,
    has_heart_rate: bool,
) {
    let groups = localized_variable_groups(
        ecliptica_data_analyzer::i18n::REPORT_VARIABLE_GROUPS,
        language,
        has_heart_rate,
    );
    variable_help_groups(ui, &groups, clipboard, toast_state, language);
}

fn heart_rate_auxiliary_panel(
    ui: &mut egui::Ui,
    enabled: &mut bool,
    clipboard: &mut Option<Clipboard>,
    toast_state: &mut ToastState,
    language: Language,
    has_heart_rate: bool,
) {
    let width = ui.available_width();
    egui_shadcn::Card::new().show(ui, |ui| {
        ui.set_min_width((width - 34.0).max(120.0));
        heart_rate_title_row(ui, language);
        ui.add_space(UI_SPACE_2);
        Switch::new(enabled)
            .label(text::ENABLE_HEART_RATE.get(language))
            .show(ui)
            .on_hover_text(text::ENABLE_HEART_RATE_HINT.get(language));
        ui.add_space(UI_SPACE_2);
        let groups = localized_variable_groups(
            ecliptica_data_analyzer::i18n::HEART_RATE_VARIABLE_GROUPS,
            language,
            has_heart_rate,
        );
        if let Some(group) = groups.first() {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(UI_SPACE_2, UI_SPACE_2);
                for variable in &group.variables {
                    variable_chip(ui, variable, group.color, clipboard, toast_state, language);
                }
            });
        }
    });
}

fn heart_rate_title_row(ui: &mut egui::Ui, language: Language) -> (egui::Response, egui::Response) {
    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width(), 24.0),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            let title = Typography::new(text::HEART_RATE_AUXILIARY.get(language))
                .font_size(16.0)
                .strong()
                .color(SETTINGS_HEADING)
                .show(ui);
            let help = ShadcnButton::icon_only(LucideIcon::CircleQuestionMark)
                .variant(ButtonVariant::Ghost)
                .size(ComponentSize::Xs)
                .show(ui)
                .on_hover_text(text::HEART_RATE_SETUP_GUIDE.get(language));
            if help.clicked() {
                open_heart_rate_guide(ui.ctx());
            }
            (title, help)
        },
    )
    .inner
}

fn open_heart_rate_guide(context: &egui::Context) {
    context.open_url(egui::OpenUrl::new_tab(HEART_RATE_GUIDE_URL));
}

fn template_help_button(ui: &mut egui::Ui, template_help_open: &mut bool, language: Language) {
    ui.add_space(UI_SPACE_2);
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        if ShadcnButton::new(text::TEMPLATE_SYNTAX_HELP.get(language))
            .icon(LucideIcon::BookOpenText)
            .variant(ButtonVariant::Outline)
            .size(ComponentSize::Xs)
            .show(ui)
            .clicked()
        {
            *template_help_open = true;
        }
    });
}

fn save_error_detail_dialog(ui: &mut egui::Ui, error: &str, language: Language) {
    Alert::new()
        .variant(AlertVariant::Destructive)
        .full_width()
        .show(ui, |ui| {
            Typography::new(text::SAVE_ERROR_GUIDANCE.get(language))
                .color(SETTINGS_DANGER)
                .wrap()
                .show(ui);
        });
    ui.add_space(UI_SPACE_3);
    ShadcnScrollArea::new(380.0)
        .id_salt("save-error-detail-scroll")
        .framed(true)
        .fill_available(true)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.add(
                egui::Label::new(
                    egui::RichText::new(error)
                        .monospace()
                        .size(13.0)
                        .color(SETTINGS_TEXT),
                )
                .wrap()
                .selectable(true),
            );
        });
}

fn template_syntax_help(ui: &mut egui::Ui, language: Language, height: f32) {
    ShadcnScrollArea::new(height)
        .id_salt("template-syntax-help-scroll")
        .framed(false)
        .fill_available(true)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for (index, example) in ecliptica_data_analyzer::i18n::TEMPLATE_SYNTAX_EXAMPLES
                .iter()
                .enumerate()
            {
                if index > 0 {
                    ui.add_space(UI_SPACE_3);
                }
                syntax_example(ui, example.title.get(language), example.code.get(language));
            }
        });
}

fn syntax_example(ui: &mut egui::Ui, title: &str, code: &str) {
    Typography::new(title)
        .strong()
        .color(SETTINGS_TEXT)
        .show(ui);
    ui.add_space(UI_SPACE_1);
    preview_panel(ui, |ui| {
        Typography::new(code)
            .monospace()
            .font_size(13.0)
            .line_height(20.0)
            .color(SETTINGS_TEXT)
            .wrap()
            .show(ui);
    });
}

fn preview_text(ui: &mut egui::Ui, preview: &str) {
    ui.add(
        egui::Label::new(preview_text_layout_job(preview))
            .wrap()
            .selectable(true),
    );
}

fn preview_text_layout_job(preview: &str) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    job.append(
        preview,
        0.0,
        egui::TextFormat {
            font_id: egui::FontId::monospace(14.0),
            color: SETTINGS_TEXT,
            line_height: Some(20.0),
            // epaint otherwise bottom-aligns glyphs inside an explicit line
            // height, which leaves the selection area looking bottom-heavy.
            valign: egui::Align::Center,
            ..Default::default()
        },
    );
    job
}

fn preview_panel(ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui)) -> egui::Response {
    let available_width = ui.available_width();
    egui::Frame::new()
        .fill(SETTINGS_PREVIEW_BG)
        .inner_margin(egui::Margin {
            left: UI_SPACE_3 as i8,
            right: UI_SPACE_3 as i8,
            top: UI_SPACE_2 as i8,
            bottom: UI_SPACE_2 as i8,
        })
        .corner_radius(6.0)
        .stroke(egui::Stroke::new(1.0, SETTINGS_INSET_BORDER))
        .show(ui, |ui| {
            ui.set_width((available_width - 26.0).max(1.0));
            content(ui);
        })
        .response
}

fn inset_surface(
    ui: &mut egui::Ui,
    fill: egui::Color32,
    content: impl FnOnce(&mut egui::Ui),
) -> egui::Response {
    let available_width = ui.available_width();
    egui::Frame::NONE
        .fill(fill)
        .inner_margin(egui::Margin::same(UI_SPACE_1 as i8))
        .corner_radius(8.0)
        .stroke(egui::Stroke::new(1.0, SETTINGS_INSET_BORDER))
        .show(ui, |ui| {
            ui.set_min_width((available_width - UI_SPACE_2 - 2.0).max(1.0));
            content(ui);
        })
        .response
}

fn page_heading(ui: &mut egui::Ui, title: &str) {
    Typography::h3(title).color(SETTINGS_HEADING).show(ui);
    ui.add_space(UI_SPACE_4);
}

fn section_card(
    ui: &mut egui::Ui,
    title: &str,
    description: Option<&str>,
    content: impl FnOnce(&mut egui::Ui),
) {
    section_card_with_status(ui, title, description, None, content);
}

fn section_card_with_status(
    ui: &mut egui::Ui,
    title: &str,
    description: Option<&str>,
    status: Option<&str>,
    content: impl FnOnce(&mut egui::Ui),
) {
    let width = ui.available_width();
    egui_shadcn::Card::new().show(ui, |ui| {
        ui.set_min_width((width - 34.0).max(120.0));
        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), 24.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                Typography::new(title)
                    .font_size(16.0)
                    .strong()
                    .color(SETTINGS_HEADING)
                    .show(ui);
                if let Some(status) = status {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        Badge::new(status).variant(BadgeVariant::Secondary).show(ui);
                    });
                }
            },
        );
        if let Some(description) = description {
            Typography::muted(description)
                .color(SETTINGS_TEXT_MUTED)
                .show(ui);
            ui.add_space(UI_SPACE_2);
        } else {
            ui.add_space(UI_SPACE_2);
        }
        content(ui);
    });
}

fn dashboard_stat(ui: &mut egui::Ui, label: &str, value: &str, color: egui::Color32) {
    let width = ui.available_width();
    egui_shadcn::Card::new().show(ui, |ui| {
        ui.set_min_width((width - 34.0).max(80.0));
        ui.set_min_height(51.0);
        Typography::new(label)
            .color(SETTINGS_TEXT_SECONDARY)
            .show(ui);
        ui.add_space(UI_SPACE_1);
        Typography::new(short_text(value, 16))
            .variant(TypographyVariant::H4)
            .color(color)
            .truncate()
            .show(ui);
    });
}

fn report_stat(ui: &mut egui::Ui, label: &str, value: &str, color: egui::Color32) {
    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width(), 34.0),
        egui::Layout::top_down(egui::Align::LEFT),
        |ui| {
            Typography::small(label)
                .color(SETTINGS_TEXT_SECONDARY)
                .wrap()
                .show(ui);
        },
    );
    Typography::new(value)
        .variant(TypographyVariant::Large)
        .color(color)
        .truncate()
        .show(ui);
}

struct ReportStatItem<'a> {
    label: &'a str,
    value: String,
    color: egui::Color32,
}

fn report_stat_group(ui: &mut egui::Ui, items: &[ReportStatItem<'_>], max_columns: usize) {
    let columns = report_stat_column_count(ui.available_width(), max_columns);
    for (row_index, row) in items.chunks(columns).enumerate() {
        if row_index > 0 {
            ui.add_space(UI_SPACE_3);
        }
        ui.columns(columns, |column_uis| {
            for (column, item) in column_uis.iter_mut().zip(row) {
                report_stat(column, item.label, &item.value, item.color);
            }
        });
    }
}

fn report_stat_column_count(available_width: f32, max_columns: usize) -> usize {
    const MIN_STAT_WIDTH: f32 = 150.0;
    const COLUMN_GAP: f32 = UI_SPACE_2;
    (((available_width + COLUMN_GAP) / (MIN_STAT_WIDTH + COLUMN_GAP)).floor() as usize)
        .clamp(1, max_columns.max(1))
}

fn log_source_row(ui: &mut egui::Ui, source: Option<&str>, runtime: &Runtime, language: Language) {
    let directory = source.and_then(|value| log_directory(Path::new(value)).ok());
    let can_open = directory.is_some();
    let hover_text = if source.is_none() {
        text::LOG_NOT_FOUND.get(language)
    } else if !can_open {
        text::LOG_MISSING.get(language)
    } else if cfg!(target_os = "macos") {
        text::OPEN_LOG_FOLDER_FINDER.get(language)
    } else if cfg!(target_os = "windows") {
        text::OPEN_LOG_FOLDER_EXPLORER.get(language)
    } else {
        text::OPEN_LOG_FOLDER_MANAGER.get(language)
    };

    PropertyRow::new(text::LOG_FILE.get(language))
        .align_start()
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.set_width(ui.available_width());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let open = ShadcnButton::new(text::OPEN_FOLDER.get(language))
                        .icon(LucideIcon::FolderOpen)
                        .variant(ButtonVariant::Outline)
                        .enabled(can_open)
                        .show(ui)
                        .on_hover_text(hover_text);

                    ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
                        Typography::new(source.unwrap_or(text::SEARCHING.get(language)))
                            .monospace()
                            .font_size(13.0)
                            .line_height(20.0)
                            .wrap()
                            .show(ui);
                    });

                    if open.clicked() {
                        let directory =
                            directory.expect("enabled button must have a valid directory");
                        match open_directory(&directory) {
                            Ok(()) => runtime.shared.event(
                                EventLevel::Info,
                                format!(
                                    "{}: {}",
                                    text::OPENED_LOG_FOLDER.get(language),
                                    directory.display()
                                ),
                            ),
                            Err(error) => runtime.shared.event(
                                EventLevel::Error,
                                format!(
                                    "{}: {error:#}",
                                    text::OPEN_LOG_FOLDER_FAILED.get(language)
                                ),
                            ),
                        }
                    }
                });
            });
        });
}

fn log_directory(path: &Path) -> anyhow::Result<PathBuf> {
    let language = Language::system_default();
    let metadata = std::fs::metadata(path).with_context(|| {
        format!(
            "{} {}",
            text::LOG_ACCESS_FAILED.get(language),
            path.display()
        )
    })?;
    anyhow::ensure!(
        metadata.is_file(),
        "{}: {}",
        text::LOG_PATH_NOT_FILE.get(language),
        path.display()
    );

    // Besides making relative paths absolute, canonicalization converts paths
    // to the platform's native representation (notably `\` separators on
    // Windows) before they are handed to the system file manager.
    let absolute_path = std::fs::canonicalize(path).with_context(|| {
        format!(
            "{} {}",
            text::LOG_ACCESS_FAILED.get(language),
            path.display()
        )
    })?;
    let directory = absolute_path.parent().ok_or_else(|| {
        anyhow::anyhow!(
            "{}: {}",
            text::LOG_PARENT_MISSING.get(language),
            path.display()
        )
    })?;
    anyhow::ensure!(
        directory.is_dir(),
        "{}: {}",
        text::LOG_FOLDER_MISSING.get(language),
        directory.display()
    );
    Ok(directory.to_owned())
}

fn open_directory(directory: &Path) -> anyhow::Result<()> {
    let language = Language::system_default();
    anyhow::ensure!(
        directory.is_dir(),
        "{}: {}",
        text::LOG_FOLDER_MISSING.get(language),
        directory.display()
    );

    #[cfg(target_os = "windows")]
    {
        open_directory_windows(directory)
    }

    #[cfg(not(target_os = "windows"))]
    {
        #[cfg(target_os = "macos")]
        let mut command = Command::new("open");
        #[cfg(not(target_os = "macos"))]
        let mut command = Command::new("xdg-open");

        command.arg(directory).spawn().with_context(|| {
            format!(
                "{} {}",
                text::FILE_MANAGER_LAUNCH_FAILED.get(language),
                directory.display()
            )
        })?;
        Ok(())
    }
}

#[cfg(target_os = "windows")]
fn open_directory_windows(directory: &Path) -> anyhow::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL};

    let language = Language::system_default();
    let operation = "open\0".encode_utf16().collect::<Vec<_>>();
    let path = directory
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    // ShellExecuteW receives the directory as a UTF-16 path, so forward
    // slashes, spaces, commas, and non-ASCII characters never pass through
    // explorer.exe's command-line option parser.
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            operation.as_ptr(),
            path.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    } as isize;
    anyhow::ensure!(
        result > 32,
        "{} {} (ShellExecuteW error {result})",
        text::FILE_MANAGER_LAUNCH_FAILED.get(language),
        directory.display()
    );
    Ok(())
}

fn overlay_height(has_report: bool, has_alert: bool) -> f32 {
    match (has_report, has_alert) {
        (true, true) => OVERLAY_REPORT_WITH_ALERT_HEIGHT,
        (true, false) | (false, true) => OVERLAY_EXPANDED_HEIGHT,
        (false, false) => OVERLAY_COMPACT_HEIGHT,
    }
}

fn overlay_header(
    ui: &mut egui::Ui,
    heart_rate: Option<u16>,
    scale: f32,
    language: Language,
) -> (egui::Response, Option<egui::Response>) {
    let right = ui.max_rect().right();
    let row = ui
        .horizontal(|ui| {
            ui.colored_label(
                OVERLAY_STATUS_DOT_COLOR,
                egui::RichText::new("●").size(9.0 * scale),
            );
            ui.label(
                egui::RichText::new(format!("ECLIPTICA  v{APP_VERSION}"))
                    .strong()
                    .size(14.0 * scale)
                    .color(egui::Color32::WHITE),
            );
        })
        .response;
    let tag = heart_rate.map(|heart_rate| {
        overlay_heart_rate_tag(ui, heart_rate, row.rect.center().y, right, scale, language)
    });
    (row, tag)
}

fn overlay_heart_rate_tag(
    ui: &mut egui::Ui,
    heart_rate: u16,
    center_y: f32,
    right: f32,
    scale: f32,
    language: Language,
) -> egui::Response {
    let color = METRIC_HEART_RATE;
    let galley = ui.painter().layout_no_wrap(
        format!("♥ {heart_rate}"),
        egui::FontId::proportional(10.0 * scale),
        color,
    );
    let padding = egui::vec2(6.0, 2.0) * scale;
    let size = galley.size() + padding * 2.0;
    let rect = egui::Rect::from_center_size(egui::pos2(right - size.x / 2.0, center_y), size);
    let painter = ui.painter();
    painter.rect_filled(
        rect,
        6.0 * scale,
        egui::Color32::from_rgba_unmultiplied(255, 105, 125, 30),
    );
    painter.galley(rect.min + padding, galley, color);
    ui.interact(rect, ui.next_auto_id(), egui::Sense::hover())
        .on_hover_text(text::HEART_RATE_AUXILIARY.get(language))
}

fn overlay_stat(
    ui: &mut egui::Ui,
    label: &str,
    full_label: &str,
    display_value: &str,
    exact_value: &str,
    color: egui::Color32,
    scale: f32,
    language: Language,
) -> egui::Response {
    let width = ui.available_width();
    egui::Frame::new()
        .fill(egui::Color32::from_rgba_unmultiplied(30, 41, 60, 145))
        .corner_radius(9.0 * scale)
        .inner_margin(egui::Margin::symmetric(
            scaled_overlay_margin(4, scale),
            scaled_overlay_margin(6, scale),
        ))
        .show(ui, |ui| {
            ui.set_width(overlay_card_inner_width(width, 4.0 * scale, scale));
            ui.label(
                egui::RichText::new(label)
                    .size(9.0 * scale)
                    .color(TEXT_SECONDARY),
            );
            ui.add(
                egui::Label::new(
                    egui::RichText::new(display_value)
                        .size(18.0 * scale)
                        .strong()
                        .color(color),
                )
                .truncate(),
            )
            .on_hover_text(format!(
                "{full_label}\n{}: {exact_value}",
                text::EXACT_VALUE.get(language)
            ));
        })
        .response
}

fn overlay_report_stat(
    ui: &mut egui::Ui,
    label: &str,
    display_value: &str,
    exact_value: &str,
    color: egui::Color32,
    scale: f32,
    language: Language,
) -> egui::Response {
    let width = ui.available_width();
    egui::Frame::new()
        .fill(egui::Color32::from_rgba_unmultiplied(30, 41, 60, 145))
        .corner_radius(9.0 * scale)
        .inner_margin(egui::Margin::symmetric(
            scaled_overlay_margin(6, scale),
            scaled_overlay_margin(5, scale),
        ))
        .show(ui, |ui| {
            ui.set_width(overlay_card_inner_width(width, 6.0 * scale, scale));
            ui.label(
                egui::RichText::new(label)
                    .size(9.0 * scale)
                    .color(TEXT_SECONDARY),
            );
            ui.add(
                egui::Label::new(
                    egui::RichText::new(display_value)
                        .size(17.0 * scale)
                        .strong()
                        .color(color),
                )
                .truncate(),
            )
            .on_hover_text(format!(
                "{}: {exact_value}",
                text::EXACT_VALUE.get(language)
            ));
        })
        .response
}

fn overlay_card_inner_width(column_width: f32, horizontal_margin: f32, scale: f32) -> f32 {
    // Preserve the original one-point rounding allowance as the whole Overlay scales.
    (column_width - horizontal_margin * 2.0 - scale).max(scale)
}

fn scaled_overlay_margin(base: i8, scale: f32) -> i8 {
    (f32::from(base) * scale).round().clamp(0.0, i8::MAX as f32) as i8
}

fn compact_u64(value: u64, language: Language) -> String {
    compact_metric(value as f64, false, language)
}

fn compact_f64(value: f64, language: Language) -> String {
    compact_metric(value, true, language)
}

fn compact_metric(value: f64, keep_decimal: bool, language: Language) -> String {
    let (
        large_threshold,
        large_divisor,
        large_suffix,
        compact_threshold,
        compact_divisor,
        compact_suffix,
    ) = match language {
        Language::English => (
            1_000_000.0,
            1_000_000.0,
            text::COMPACT_HUNDRED_MILLION.get(language),
            1_000.0,
            1_000.0,
            text::COMPACT_TEN_THOUSAND.get(language),
        ),
        Language::Chinese => (
            100_000_000.0,
            100_000_000.0,
            text::COMPACT_HUNDRED_MILLION.get(language),
            10_000.0,
            10_000.0,
            text::COMPACT_TEN_THOUSAND.get(language),
        ),
    };
    if value.abs() < compact_threshold {
        return if keep_decimal {
            format!("{value:.1}")
        } else {
            format!("{value:.0}")
        };
    }
    let (scaled, suffix) = if value.abs() >= large_threshold {
        (value / large_divisor, large_suffix)
    } else {
        (value / compact_divisor, compact_suffix)
    };
    let formatted = if scaled.abs() >= 100.0 {
        format!("{scaled:.0}")
    } else {
        format!("{scaled:.1}")
    };
    format!("{}{}", formatted.trim_end_matches(".0"), suffix)
}

fn lock_card(
    ui: &mut egui::Ui,
    lock: &str,
    locked_self: bool,
    scale: f32,
    language: Language,
) -> (
    egui::Response,
    egui::Response,
    egui::Response,
    egui::Response,
) {
    let color = if locked_self {
        METRIC_DAMAGE_TAKEN
    } else {
        METRIC_BOSS_LOCK
    };
    let width = ui.available_width();
    let frame = egui::Frame::new()
        .fill(color.gamma_multiply(if locked_self { 0.16 } else { 0.08 }))
        .corner_radius(10.0 * scale)
        .inner_margin(egui::Margin::symmetric(
            scaled_overlay_margin(11, scale),
            scaled_overlay_margin(8, scale),
        ))
        .show(ui, |ui| {
            ui.set_width(overlay_card_inner_width(width, 11.0 * scale, scale));
            let row_size = egui::vec2(ui.available_width(), ui.spacing().interact_size.y);
            ui.allocate_ui_with_layout(
                row_size,
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    let label = ui.add(
                        egui::Label::new(overlay_lock_label_text(language, scale))
                            .selectable(false),
                    );
                    let value = ui
                        .with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(short_text(lock, 22))
                                        .size(16.0 * scale)
                                        .strong()
                                        .color(color),
                                )
                                .truncate()
                                .selectable(false),
                            )
                        })
                        .inner;
                    (label, value)
                },
            )
        });
    let row = frame.inner;
    (frame.response, row.response, row.inner.0, row.inner.1)
}

fn overlay_lock_label_text(language: Language, scale: f32) -> egui::RichText {
    let text = egui::RichText::new(text::BOSS_LOCK.get(language)).color(TEXT_SECONDARY);
    if let Some(font) = overlay_lock_label_font(language, scale) {
        // Render the Latin and Han glyphs with one font so `BOSS` and `锁定`
        // share the same metrics instead of sitting on mismatched fallback baselines.
        text.font(font)
    } else {
        text.size(10.0 * scale)
    }
}

fn overlay_lock_label_font(language: Language, scale: f32) -> Option<egui::FontId> {
    (language == Language::Chinese)
        .then(|| egui::FontId::new(10.0 * scale, egui::FontFamily::Name(CJK_FONT_FAMILY.into())))
}

fn log_line(ui: &mut egui::Ui, row: &LogRow, language: Language) -> egui::Response {
    let (level, variant) = match row.level {
        EventLevel::Info => (text::INFO.get(language), BadgeVariant::Info),
        EventLevel::Warning => (text::WARNING.get(language), BadgeVariant::Warning),
        EventLevel::Error => (text::ERROR.get(language), BadgeVariant::Destructive),
    };
    let item_width = ui.available_width();
    let response = Item::new().show(ui, |ui| {
        // Keep log rows flat inside the surrounding section card. Item still
        // provides row padding, but its default border is transparent.
        ui.set_width((item_width - 26.0).max(120.0));
        Flex::row()
            .align_center()
            .gap(8.0)
            .w_full()
            .show(ui, |flex| {
                flex.ui(|ui| {
                    Typography::small(&row.time)
                        .monospace()
                        .color(SETTINGS_TEXT_SECONDARY)
                        .show(ui);
                });
                flex.ui(|ui| {
                    Badge::new(level).variant(variant).show(ui);
                });
                if row.repeats > 1 {
                    flex.spacer();
                    flex.ui(|ui| {
                        Badge::new(format!("×{}", row.repeats))
                            .variant(BadgeVariant::Secondary)
                            .show(ui);
                    });
                }
            });
        ui.add_space(UI_SPACE_1);
        Typography::new(&row.message)
            .color(SETTINGS_TEXT)
            .line_height(20.0)
            .wrap()
            .show(ui);
    });
    ui.add_space(8.0);
    response
}

fn is_protocol_diagnostic(message: &str) -> bool {
    Language::ALL.into_iter().any(|language| {
        message.starts_with(&format!("{} [", text::LOG_PROTOCOL_DEGRADED.get(language)))
    })
}

fn event_creates_overlay_alert(event: &SystemEvent) -> bool {
    event.presentation == EventPresentation::Default && event.level != EventLevel::Info
}

fn toast_variant_for_event(level: ToastLevel) -> ToastVariant {
    match level {
        ToastLevel::Success => ToastVariant::Success,
        ToastLevel::Warning => ToastVariant::Warning,
        ToastLevel::Error => ToastVariant::Error,
    }
}

fn register_hidden_click(clicks: &mut u8, last_click: &mut Option<Instant>, now: Instant) -> bool {
    if last_click.is_none_or(|last| now.duration_since(last) > DEVELOPER_MODE_CLICK_TIMEOUT) {
        *clicks = 0;
    }
    *last_click = Some(now);
    *clicks = clicks.saturating_add(1);
    if *clicks < DEVELOPER_MODE_CLICK_COUNT {
        return false;
    }
    *clicks = 0;
    true
}

fn push_log_row(logs: &mut VecDeque<LogRow>, event: SystemEvent) {
    if let Some(last) = logs.back_mut() {
        if last.message == event.message && last.level == event.level {
            last.repeats = last.repeats.saturating_add(1);
            return;
        }
    }
    logs.push_back(LogRow {
        time: chrono::Local::now().format("%H:%M:%S").to_string(),
        level: event.level,
        message: event.message,
        repeats: 1,
    });
    while logs.len() > MAX_LOG_ROWS {
        logs.pop_front();
    }
}

fn settings_status_pill(ui: &mut egui::Ui, status: DataStatus, language: Language) {
    ui.allocate_ui_with_layout(
        egui::vec2(SETTINGS_STATUS_SLOT_WIDTH, 22.0),
        egui::Layout::right_to_left(egui::Align::Center),
        |ui| {
            let label = match status {
                DataStatus::Searching => text::STATUS_SEARCHING.get(language),
                DataStatus::Recovering => text::STATUS_RECOVERING.get(language),
                DataStatus::Live => text::STATUS_LIVE.get(language),
                DataStatus::Stale => text::STATUS_STALE.get(language),
                DataStatus::Error => text::STATUS_ERROR.get(language),
            };
            Badge::new(label)
                .variant(status_badge_variant(status))
                .show(ui);
        },
    );
}

fn status_badge_variant(status: DataStatus) -> BadgeVariant {
    match status {
        DataStatus::Live => BadgeVariant::Success,
        DataStatus::Searching | DataStatus::Recovering => BadgeVariant::Info,
        DataStatus::Stale => BadgeVariant::Warning,
        DataStatus::Error => BadgeVariant::Destructive,
    }
}

fn short_text(value: &str, limit: usize) -> String {
    let mut chars = value.chars();
    let shortened: String = chars.by_ref().take(limit).collect();
    if chars.next().is_some() {
        format!("{shortened}…")
    } else {
        shortened
    }
}

fn message_template_draft_changed(draft: &AppConfig, applied: &AppConfig) -> bool {
    let selected = draft.active_message_template_preset;
    draft.active_message_template_preset != applied.active_message_template_preset
        || draft.message_template != applied.message_template
        || draft.message_template_preset_names != applied.message_template_preset_names
        || draft
            .message_template_presets
            .get(selected)
            .is_none_or(|slot| slot != &draft.message_template)
        || draft.message_template_presets != applied.message_template_presets
}

fn report_template_draft_changed(draft: &AppConfig, applied: &AppConfig) -> bool {
    let selected = draft.active_round_report_template_preset;
    draft.active_round_report_template_preset != applied.active_round_report_template_preset
        || draft.round_report_template != applied.round_report_template
        || draft.round_report_template_preset_names != applied.round_report_template_preset_names
        || draft
            .round_report_template_presets
            .get(selected)
            .is_none_or(|slot| slot != &draft.round_report_template)
        || draft.round_report_template_presets != applied.round_report_template_presets
}

fn preset_tab_labels(names: &[String], language: Language) -> Vec<String> {
    names
        .iter()
        .enumerate()
        .map(|(index, name)| {
            short_text(
                &preset_display_name(name, index, language),
                TEMPLATE_PRESET_TAB_LABEL_MAX_CHARS,
            )
        })
        .collect()
}

fn preset_display_name(name: &str, index: usize, language: Language) -> String {
    let name = name.trim();
    if name.is_empty() {
        format_pattern(
            text::PRESET_FALLBACK,
            language,
            &[("index", (index + 1).to_string())],
        )
    } else {
        name.to_owned()
    }
}

fn preset_controls_row<R>(
    ui: &mut egui::Ui,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width(), TEMPLATE_PRESET_TAB_ROW_HEIGHT),
        egui::Layout::left_to_right(egui::Align::Center),
        |row| {
            // The application-wide minimum interaction height is 36 px. This
            // compact row intentionally uses 28 px controls; matching the
            // row's interaction metric prevents egui's placer from applying
            // different baseline offsets to the framed tabs and the button.
            row.spacing_mut().interact_size.y = TEMPLATE_PRESET_TAB_ROW_HEIGHT;
            add_contents(row)
        },
    )
}

fn self_lock_transition(
    previous: &GameSnapshot,
    current: &GameSnapshot,
    display_name: &str,
) -> Option<SelfLockTransition> {
    if display_name.trim().is_empty()
        || !matches!(previous.status, DataStatus::Live | DataStatus::Stale)
        || current.status != DataStatus::Live
    {
        return None;
    }

    let display_name = normalized_name(display_name);
    let was_self = previous
        .boss_lock
        .as_deref()
        .is_some_and(|name| normalized_name(name) == display_name);
    let is_self = current
        .boss_lock
        .as_deref()
        .is_some_and(|name| normalized_name(name) == display_name);

    if current.boss_active && is_self && !was_self {
        return Some(SelfLockTransition::Locked);
    }

    let same_active_boss = previous.boss_active
        && current.boss_active
        && previous
            .boss
            .as_deref()
            .zip(current.boss.as_deref())
            .is_some_and(|(previous, current)| {
                normalized_name(previous) == normalized_name(current)
            });
    let explicitly_transferred_to_other = current
        .boss_lock
        .as_deref()
        .is_some_and(|name| normalized_name(name) != display_name);

    (same_active_boss && was_self && explicitly_transferred_to_other)
        .then_some(SelfLockTransition::Unlocked)
}

fn install_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.override_text_color = Some(SETTINGS_TEXT);
    visuals.weak_text_alpha = 1.0;
    visuals.weak_text_color = Some(SETTINGS_TEXT_MUTED);
    visuals.panel_fill = SETTINGS_BG;
    visuals.window_fill = SETTINGS_SURFACE;
    visuals.extreme_bg_color = egui::Color32::from_rgb(11, 9, 15);
    visuals.faint_bg_color = SETTINGS_SURFACE_HOVER;
    visuals.widgets.inactive.bg_fill = SETTINGS_SURFACE;
    visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, SETTINGS_BORDER);
    visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, SETTINGS_TEXT);
    visuals.widgets.hovered.bg_fill = SETTINGS_SURFACE_HOVER;
    visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, SETTINGS_ACCENT);
    visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, SETTINGS_TEXT);
    visuals.widgets.active.bg_fill = egui::Color32::from_rgb(48, 39, 68);
    visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, SETTINGS_ACCENT);
    visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, SETTINGS_TEXT);
    visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, SETTINGS_BORDER);
    visuals.hyperlink_color = SETTINGS_ACCENT;
    visuals.warn_fg_color = egui::Color32::from_rgb(255, 207, 112);
    visuals.error_fg_color = egui::Color32::from_rgb(255, 132, 146);
    visuals.selection.bg_fill = egui::Color32::from_rgb(74, 61, 109);
    visuals.selection.stroke = egui::Stroke::new(1.0, SETTINGS_TEXT);
    visuals.window_corner_radius = 6.0.into();
    ctx.set_visuals(visuals);

    ctx.style_mut(|style| {
        style.spacing.item_spacing = egui::vec2(UI_SPACE_2, UI_SPACE_2);
        style.spacing.button_padding = egui::vec2(12.0, 8.0);
        style.spacing.interact_size.y = 36.0;
    });

    let mut shadcn = egui_shadcn::theme::shadcn_theme_dark::dark();
    shadcn.background = SETTINGS_SURFACE;
    shadcn.foreground = SETTINGS_TEXT;
    shadcn.card = SETTINGS_SURFACE;
    shadcn.card_foreground = SETTINGS_TEXT;
    shadcn.popover = SETTINGS_SURFACE;
    shadcn.popover_foreground = SETTINGS_TEXT;
    shadcn.primary = SETTINGS_ACCENT;
    shadcn.primary_foreground = SETTINGS_BG;
    shadcn.secondary = SETTINGS_SURFACE_HOVER;
    shadcn.secondary_foreground = SETTINGS_TEXT;
    shadcn.muted = SETTINGS_SURFACE_HOVER;
    shadcn.muted_foreground = SETTINGS_TEXT_MUTED;
    shadcn.accent = egui::Color32::from_rgb(47, 39, 67);
    shadcn.accent_foreground = egui::Color32::from_rgb(223, 214, 255);
    shadcn.border = SETTINGS_BORDER;
    shadcn.input = egui::Color32::from_rgb(76, 67, 94);
    shadcn.ring = SETTINGS_ACCENT;
    shadcn.radius = 6.0;
    ctx.set_shadcn_theme(shadcn);
}

fn install_cjk_font(ctx: &egui::Context) {
    let cjk_candidates: &[&str] = if cfg!(target_os = "windows") {
        &["C:/Windows/Fonts/msyh.ttc", "C:/Windows/Fonts/simhei.ttf"]
    } else {
        &[
            "/System/Library/Fonts/PingFang.ttc",
            "/System/Library/Fonts/STHeiti Light.ttc",
        ]
    };
    let extended_text_candidates: &[&str] = if cfg!(target_os = "windows") {
        &["C:/Windows/Fonts/segoeui.ttf", "C:/Windows/Fonts/arial.ttf"]
    } else {
        &[
            "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
            "/System/Library/Fonts/Helvetica.ttc",
        ]
    };
    let symbol_candidates: &[&str] = if cfg!(target_os = "windows") {
        &[
            "C:/Windows/Fonts/seguisym.ttf",
            "C:/Windows/Fonts/seguiemj.ttf",
        ]
    } else {
        &[
            "/System/Library/Fonts/Apple Symbols.ttf",
            "/System/Library/Fonts/CJKSymbolsFallback.ttc",
        ]
    };

    let mut fonts = egui::FontDefinitions::default();
    if !install_font_fallback(&mut fonts, CJK_FONT_FAMILY, cjk_candidates) {
        tracing::warn!("未找到系统 CJK 字体，中文可能显示为方框");
    }
    if !install_font_fallback(
        &mut fonts,
        EXTENDED_TEXT_FONT_FAMILY,
        extended_text_candidates,
    ) {
        tracing::warn!("未找到扩展字符字体，玩家名中的特殊字符可能显示为方框");
    }
    if !install_font_fallback(&mut fonts, SYMBOL_FONT_FAMILY, symbol_candidates) {
        tracing::warn!("未找到系统符号字体，玩家名中的装饰符号可能显示为方框");
    }
    ctx.set_fonts(fonts);
}

fn install_font_fallback(
    fonts: &mut egui::FontDefinitions,
    family_name: &'static str,
    candidates: &[&str],
) -> bool {
    let Some(bytes) = candidates.iter().find_map(|path| std::fs::read(path).ok()) else {
        return false;
    };
    fonts.font_data.insert(
        family_name.to_owned(),
        egui::FontData::from_owned(bytes).into(),
    );
    append_font_fallback(fonts, family_name);
    true
}

fn append_font_fallback(fonts: &mut egui::FontDefinitions, family_name: &'static str) {
    fonts.families.insert(
        egui::FontFamily::Name(family_name.into()),
        vec![family_name.to_owned()],
    );
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts
            .families
            .entry(family)
            .or_default()
            .push(family_name.to_owned());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_only_update_preserves_every_other_committed_setting() {
        let committed = AppConfig {
            language: Language::Chinese,
            display_name: "Committed name".to_owned(),
            osc_address: "192.0.2.1:9000".to_owned(),
            overlay_scale: 1.75,
            ..AppConfig::default()
        };

        let updated = config_with_language(&committed, Language::English);
        assert_eq!(updated.language, Language::English);
        assert_eq!(updated.display_name, committed.display_name);
        assert_eq!(updated.osc_address, committed.osc_address);
        assert_eq!(updated.overlay_scale, committed.overlay_scale);
        assert_eq!(updated.send_interval, committed.send_interval);
        assert_eq!(updated.alert_volume, committed.alert_volume);
    }

    #[test]
    fn language_only_update_does_not_commit_an_unrelated_draft_edit() {
        let committed = AppConfig {
            language: Language::Chinese,
            display_name: "Saved name".to_owned(),
            ..AppConfig::default()
        };
        let mut draft = committed.clone();
        draft.display_name = "Unsaved draft name".to_owned();

        let persisted = config_with_language(&committed, Language::English);
        let draft = config_with_language(&draft, Language::English);

        assert_eq!(persisted.display_name, "Saved name");
        assert_eq!(draft.display_name, "Unsaved draft name");
        assert_ne!(draft, persisted, "the unrelated edit must remain a draft");
    }

    #[test]
    fn applying_language_fields_does_not_touch_unrelated_live_settings() {
        let mut live = AppConfig::defaults_for_language(Language::Chinese);
        live.overlay_x = 432.0;
        live.overlay_scale = 1.75;
        let localized = live.with_localized_defaults(Language::English);

        assert!(apply_language_managed_fields(&mut live, &localized));
        assert_eq!(live.language, Language::English);
        assert_eq!(live.overlay_x, 432.0);
        assert_eq!(live.overlay_scale, 1.75);
        assert_eq!(
            live.message_template,
            AppConfig::defaults_for_language(Language::English).message_template
        );
    }

    #[test]
    fn live_preview_forces_no_wasd_false_without_mutating_runtime_snapshot() {
        let runtime_snapshot = GameSnapshot {
            no_wasd_for_10s: true,
            ..GameSnapshot::default()
        };

        let preview = preview_snapshot_for_state(&runtime_snapshot, TemplatePreviewState::Normal);

        assert!(!preview.no_wasd_for_10s);
        assert!(runtime_snapshot.no_wasd_for_10s);
    }

    #[test]
    fn live_preview_tabs_select_distinct_simulated_template_contexts() {
        let runtime_snapshot = GameSnapshot {
            phase: RoundPhase::Combat,
            round_report: Some(RoundReport {
                has_duration_data: true,
                has_output_data: true,
                duration_seconds: 10,
                total_damage: 999,
                average_dps: 10.0,
                max_dps: 20,
                effective_dps: 12.0,
                burst_10s_dps: Some(15.0),
                dps_growth_rate: 0.0,
                has_dps_growth_rate: false,
                damage_taken: 2,
                has_longest_standstill_data: true,
                longest_standstill_seconds: 3,
            }),
            ..GameSnapshot::default()
        };
        let config = AppConfig {
            message_template: "COMBAT {{latest_dps}}".to_owned(),
            round_report_template: "REPORT {{round_total_damage}}".to_owned(),
            ..AppConfig::default()
        };

        let normal = preview_snapshot_for_state(&runtime_snapshot, TemplatePreviewState::Normal);
        let report =
            preview_snapshot_for_state(&runtime_snapshot, TemplatePreviewState::RoundReport);

        assert_eq!(normal.phase, RoundPhase::Combat);
        assert!(normal.round_metrics_active);
        assert!(normal.round_report.is_none());
        assert_eq!(report.phase, RoundPhase::Lobby);
        assert!(report.round_report.is_some());
        assert_eq!(
            ecliptica_data_analyzer::osc::render_configured_message(&config, &normal).unwrap(),
            "COMBAT 128"
        );
        assert_eq!(
            ecliptica_data_analyzer::osc::render_configured_message(&config, &report).unwrap(),
            "REPORT 999"
        );
    }

    #[test]
    fn only_protocol_miss_events_are_treated_as_developer_logs() {
        assert!(is_protocol_diagnostic(
            "日志协议兼容性降级 [damage]: 格式已变化"
        ));
        assert!(!is_protocol_diagnostic("日志读取发生错误，OSC 已暂停"));
        assert!(!is_protocol_diagnostic("未找到 VRChat 日志，将继续重试"));
    }

    #[test]
    fn toast_events_never_create_overlay_alerts() {
        for (level, toast_level, expected_variant) in [
            (EventLevel::Info, ToastLevel::Success, ToastVariant::Success),
            (
                EventLevel::Warning,
                ToastLevel::Warning,
                ToastVariant::Warning,
            ),
            (EventLevel::Error, ToastLevel::Error, ToastVariant::Error),
        ] {
            let event = SystemEvent {
                level,
                message: "heart-rate status".to_owned(),
                presentation: EventPresentation::Toast(toast_level),
            };

            assert!(!event_creates_overlay_alert(&event));
            assert_eq!(toast_variant_for_event(toast_level), expected_variant);
        }
    }

    #[test]
    fn each_developer_mode_toggle_requires_five_timely_logo_clicks() {
        let start = Instant::now();
        let mut clicks = 0;
        let mut last_click = None;
        for index in 0..4 {
            assert!(!register_hidden_click(
                &mut clicks,
                &mut last_click,
                start + Duration::from_millis(index * 300),
            ));
        }
        assert!(register_hidden_click(
            &mut clicks,
            &mut last_click,
            start + Duration::from_millis(1_200),
        ));

        for index in 1..=5 {
            let toggled = register_hidden_click(
                &mut clicks,
                &mut last_click,
                start + Duration::from_millis(1_200 + index * 300),
            );
            assert_eq!(toggled, index == 5);
        }

        assert!(!register_hidden_click(
            &mut clicks,
            &mut last_click,
            start + Duration::from_secs(10),
        ));
        assert_eq!(clicks, 1, "timed-out click sequence must restart");
    }

    #[test]
    fn log_directory_requires_an_existing_file_and_returns_its_parent() {
        let directory = tempfile::tempdir().unwrap();
        let log = directory.path().join("output_log_test.txt");
        std::fs::write(&log, b"test").unwrap();

        assert_eq!(
            log_directory(&log).unwrap(),
            std::fs::canonicalize(directory.path()).unwrap()
        );
        assert!(log_directory(&directory.path().join("missing.txt")).is_err());
        assert!(log_directory(directory.path()).is_err());
    }

    fn live_boss(name: &str, lock: Option<&str>) -> GameSnapshot {
        GameSnapshot {
            boss: Some(name.to_owned()),
            boss_lock: lock.map(str::to_owned),
            boss_active: true,
            status: DataStatus::Live,
            ..GameSnapshot::default()
        }
    }

    #[test]
    fn detects_lock_and_explicit_transfer_away() {
        let other = live_boss("Jim", Some("Other"));
        let me = live_boss("Jim", Some("Alice"));

        assert_eq!(
            self_lock_transition(&other, &me, "Alice"),
            Some(SelfLockTransition::Locked)
        );
        assert_eq!(
            self_lock_transition(&me, &other, "Alice"),
            Some(SelfLockTransition::Unlocked)
        );
    }

    #[test]
    fn detects_first_lock_when_new_log_activity_recovers_from_stale() {
        let stale = GameSnapshot {
            status: DataStatus::Stale,
            ..GameSnapshot::default()
        };
        let me = live_boss("Jim", Some("Alice"));

        assert_eq!(
            self_lock_transition(&stale, &me, "Alice"),
            Some(SelfLockTransition::Locked)
        );
    }

    #[test]
    fn startup_recovery_does_not_treat_replayed_lock_as_new() {
        let recovering = GameSnapshot {
            status: DataStatus::Recovering,
            ..GameSnapshot::default()
        };
        let me = live_boss("Jim", Some("Alice"));

        assert_eq!(self_lock_transition(&recovering, &me, "Alice"), None);
    }

    #[test]
    fn boss_death_and_phase_change_do_not_play_unlock_sound() {
        let me = live_boss("JimPhase1", Some("Alice"));
        let dead = GameSnapshot {
            status: DataStatus::Live,
            ..GameSnapshot::default()
        };
        let next_phase = live_boss("JimPhase2", Some("Other"));

        assert_eq!(self_lock_transition(&me, &dead, "Alice"), None);
        assert_eq!(self_lock_transition(&me, &next_phase, "Alice"), None);
    }

    #[test]
    fn shadcn_switch_click_reports_changed_state() {
        let ctx = egui::Context::default();
        let mut on = false;
        let mut switch_rect = egui::Rect::NOTHING;

        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                switch_rect = Switch::new(&mut on).label("可拖动").show(ui).rect;
            });
        });

        let pointer = switch_rect.center();
        let pointer_event = |pressed| egui::Event::PointerButton {
            pos: pointer,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let _ = ctx.run(
            egui::RawInput {
                events: vec![egui::Event::PointerMoved(pointer), pointer_event(true)],
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    Switch::new(&mut on).label("可拖动").show(ui);
                });
            },
        );

        let mut changed = false;
        let _ = ctx.run(
            egui::RawInput {
                events: vec![pointer_event(false)],
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    changed = Switch::new(&mut on).label("可拖动").show(ui).changed();
                });
            },
        );

        assert!(on);
        assert!(changed);
    }

    #[test]
    fn management_rows_keep_exact_columns_and_multiline_alignment() {
        egui::__run_test_ui(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(420.0, 36.0),
                egui::Layout::top_down(egui::Align::LEFT),
                |ui| {
                    ui.set_width(420.0);
                    let mut value_rect = egui::Rect::NOTHING;
                    let row = PropertyRow::new("Current phase")
                        .label_width(84.0)
                        .show(ui, |ui| {
                            value_rect = Typography::new("Lobby").show(ui).rect;
                        });

                    assert!((value_rect.left() - (row.rect.left() + 94.0)).abs() <= 0.5);
                    assert!((value_rect.center().y - row.rect.center().y).abs() <= 0.5);
                },
            );

            ui.allocate_ui_with_layout(
                egui::vec2(280.0, 20.0),
                egui::Layout::top_down(egui::Align::LEFT),
                |ui| {
                    ui.set_width(280.0);
                    let mut multiline_rect = egui::Rect::NOTHING;
                    let multiline = PropertyRow::new("Log file")
                        .label_width(84.0)
                        .align_start()
                        .show(ui, |ui| {
                            multiline_rect = Typography::new(
                                "/Users/example/Documents/ecliptica/data/output_log_\n2026-08-05_00-55-57.txt",
                            )
                            .monospace()
                            .font_size(13.0)
                            .line_height(20.0)
                            .wrap()
                            .show(ui)
                            .rect;
                        });

                    assert!(multiline.rect.height() >= 40.0);
                    assert!(
                        (multiline_rect.left() - (multiline.rect.left() + 94.0)).abs() <= 0.5
                    );
                    assert!((multiline_rect.top() - multiline.rect.top()).abs() <= 0.5);
                },
            );
        });
    }

    #[test]
    fn boss_alert_sound_controls_share_one_starting_column() {
        egui::__run_test_ui(|ui| {
            ui.set_width(460.0);
            let mut lock_control_left = 0.0;
            let mut release_control_left = 0.0;

            PropertyRow::new(text::LOCK_SOUND.get(Language::Chinese))
                .label_width(alert_sound_label_width(Language::Chinese))
                .show(ui, |ui| lock_control_left = ui.cursor().left());
            PropertyRow::new(text::RELEASE_SOUND.get(Language::Chinese))
                .label_width(alert_sound_label_width(Language::Chinese))
                .show(ui, |ui| release_control_left = ui.cursor().left());

            assert!((lock_control_left - release_control_left).abs() <= 0.5);
            assert!(
                alert_sound_label_width(Language::Chinese)
                    < alert_sound_label_width(Language::English)
            );
        });
    }

    #[test]
    fn sidebar_notice_height_follows_its_content() {
        egui::__run_test_ui(|ui| {
            ui.set_width(186.0);
            let one_line = sidebar_notice_height(ui, "设置已保存", 186.0);
            let two_lines = sidebar_notice_height(ui, "设置已保存，OSC 会自动更新", 40.0);

            assert!(one_line > 0.0);
            assert!(two_lines > one_line);
        });
    }

    #[test]
    fn floating_window_title_uses_the_selected_language() {
        assert_eq!(
            overlay_window_title(Language::Chinese),
            format!("Ecliptica 悬浮窗 v{APP_VERSION}")
        );
        assert_eq!(
            overlay_window_title(Language::English),
            format!("Ecliptica Floating Window v{APP_VERSION}")
        );
    }

    #[test]
    fn dps_chart_keeps_a_single_second_peak_exact() {
        let raw = (0..30)
            .map(|second| [second as f64, if second == 5 { 200.0 } else { 0.0 }])
            .collect::<Vec<_>>();
        let reduced = downsample_dps_trend(&raw, 600);

        assert_eq!(reduced, raw);
        assert_eq!(
            reduced.iter().map(|point| point[1]).fold(0.0, f64::max),
            200.0
        );
    }

    #[test]
    fn chart_round_context_moves_between_current_and_completed_without_changing_identity() {
        let mut snapshot = GameSnapshot {
            phase: RoundPhase::Combat,
            combat_round_epoch: 4,
            dps_history: vec![
                DpsHistoryPoint {
                    elapsed_seconds: 10,
                    dps: 100,
                    combat_round_epoch: 3,
                    estimated_step: Some(8),
                },
                DpsHistoryPoint {
                    elapsed_seconds: 20,
                    dps: 200,
                    combat_round_epoch: 4,
                    estimated_step: Some(9),
                },
            ],
            ..GameSnapshot::default()
        };

        assert_eq!(
            dps_chart_round_context(&snapshot, Language::Chinese),
            Some((4, "预计当前为第 9 回合".to_owned()))
        );

        snapshot.phase = RoundPhase::Lobby;
        assert_eq!(
            dps_chart_round_context(&snapshot, Language::Chinese),
            Some((4, "预计第 9 回合刚结束".to_owned()))
        );
    }

    #[test]
    fn chart_round_badge_does_not_add_a_vertical_layout_row() {
        egui::__run_test_ui(|ui| {
            ui.set_width(640.0);
            let before_plain = ui.cursor().top();
            section_card_with_status(ui, "DPS chart", Some("Description"), None, |_| {});
            let plain_height = ui.cursor().top() - before_plain;

            let before_status = ui.cursor().top();
            section_card_with_status(
                ui,
                "DPS chart",
                Some("Description"),
                Some("Completed round"),
                |_| {},
            );
            let status_height = ui.cursor().top() - before_status;

            assert!((plain_height - status_height).abs() <= 0.5);
        });
    }

    #[test]
    fn dps_trend_uses_the_original_slow_release_smoothing() {
        let raw = (0..30)
            .map(|second| [second as f64, if second == 5 { 900.0 } else { 0.0 }])
            .collect::<Vec<_>>();
        let trend = dps_trend_points(&raw);

        assert_eq!(trend[4][1], 0.0);
        assert!((trend[5][1] - 252.0).abs() < 1e-9);
        assert!(trend[6][1] < trend[5][1] && trend[6][1] > 0.0);
        assert!(trend[12][1] < trend[6][1]);
    }

    #[test]
    fn chart_peak_keeps_the_first_exact_maximum() {
        let points = [[0.0, 10.0], [1.0, 1_111.0], [2.0, 1_111.0]];
        assert_eq!(chart_peak(&points), Some([1.0, 1_111.0]));
    }

    #[test]
    fn chart_peak_has_a_twelve_point_hover_target() {
        let peak = egui::pos2(100.0, 100.0);
        assert!(chart_peak_is_hovered(egui::pos2(112.0, 100.0), peak));
        assert!(!chart_peak_is_hovered(egui::pos2(112.1, 100.0), peak));
    }

    #[test]
    fn chart_best_view_contains_the_full_line_with_padding() {
        let points = [[65.0, 100.0], [90.0, 150.0], [125.0, 200.0]];
        let (x, y) = chart_best_view_bounds(&points);
        assert!(x.0 < 65.0 && x.1 > 125.0);
        assert!(y.0 > 0.0 && y.0 < 100.0);
        assert!(y.1 > 200.0);
    }

    #[test]
    fn chart_best_view_uses_only_the_recent_five_minutes() {
        let points = [
            [0.0, 10_000.0],
            [399.0, 9_000.0],
            [400.0, 100.0],
            [550.0, 200.0],
            [700.0, 150.0],
        ];
        let (x, y) = chart_best_view_bounds(&points);

        assert!(x.0 <= 400.0 && x.0 > 390.0);
        assert!(x.1 > 700.0);
        assert!(y.0 < 100.0);
        assert!(y.1 > 200.0 && y.1 < 1_000.0);
    }

    #[test]
    fn chart_best_view_handles_zero_and_single_point_lines() {
        assert_eq!(chart_best_view_bounds(&[]), ((0.0, 10.0), (0.0, 1.0)));

        let (x, y) = chart_best_view_bounds(&[[0.0, 0.0]]);
        assert_eq!(x, (0.0, 5.0));
        assert_eq!(y, (0.0, DPS_CHART_MIN_Y_SPAN));

        let (x, y) = chart_best_view_bounds(&[[12.0, 250.0]]);
        assert_eq!(x, (7.0, 17.0));
        assert_eq!(y, (225.0, 275.0));
    }

    #[test]
    fn chart_y_ticks_keep_small_values_distinct() {
        let range = 0.0..=1.0;
        assert_eq!(
            format_chart_y_tick(0.2, 0.1, &range, Language::Chinese),
            "0.2"
        );
        assert_eq!(
            format_chart_y_tick(0.75, 0.25, &range, Language::Chinese),
            "0.75"
        );
        assert_eq!(
            format_chart_y_tick(1.0, 0.1, &range, Language::Chinese),
            "1"
        );
    }

    #[test]
    fn chart_auto_fit_never_collapses_valid_zero_data_to_one() {
        let points = [[0.0, 0.0], [30.0, 0.0], [60.0, 0.0]];
        let (_, y) = chart_best_view_bounds(&points);

        assert_eq!(y, (0.0, DPS_CHART_MIN_Y_SPAN));
    }

    #[test]
    fn chart_auto_fit_waits_after_user_input_and_between_adjustments() {
        let start = Instant::now();
        let mut view = DpsChartViewState::default();
        assert!(view.should_auto_fit(start));
        view.record_auto_fit(start);
        assert!(!view.should_auto_fit(start + Duration::from_secs(4)));
        assert!(view.should_auto_fit(start + DPS_CHART_AUTO_FIT_INTERVAL));

        let interaction = start + Duration::from_secs(6);
        view.record_user_interaction(interaction);
        assert!(!view.should_auto_fit(interaction + Duration::from_secs(4)));
        assert!(view.should_auto_fit(interaction + DPS_CHART_AUTO_FIT_IDLE));

        let resumed = interaction + DPS_CHART_AUTO_FIT_IDLE;
        view.record_auto_fit(resumed);
        assert!(!view.should_auto_fit(resumed + Duration::from_secs(4)));
        assert!(view.should_auto_fit(resumed + DPS_CHART_AUTO_FIT_INTERVAL));
    }

    #[test]
    fn chart_elapsed_time_is_human_readable_game_time() {
        assert_eq!(format_chart_elapsed(0.0, Language::English), "0s");
        assert_eq!(format_chart_elapsed(60.0, Language::English), "1m");
        assert_eq!(format_chart_elapsed(125.0, Language::English), "2m 05s");
        assert_eq!(format_chart_elapsed(3_900.0, Language::English), "1h 05m");
        assert_eq!(format_chart_elapsed(125.0, Language::Chinese), "2分 05秒");
    }

    #[test]
    fn chart_hover_interpolates_the_line_value_by_x_only() {
        let points = [[0.0, 100.0], [10.0, 200.0], [20.0, 120.0]];
        assert_eq!(chart_point_at_x(&points, 0.0), Some([0.0, 100.0]));
        assert_eq!(chart_point_at_x(&points, 5.0), Some([5.0, 150.0]));
        assert_eq!(chart_point_at_x(&points, 15.0), Some([15.0, 160.0]));
        assert_eq!(chart_point_at_x(&points, 21.0), None);
        assert_eq!(chart_nearest_point_at_x(&points, -0.4), Some([0.0, 100.0]));
        assert_eq!(chart_nearest_point_at_x(&points, 4.0), Some([0.0, 100.0]));
        assert_eq!(chart_nearest_point_at_x(&points, 6.0), Some([10.0, 200.0]));
        assert_eq!(chart_nearest_point_at_x(&points, 20.6), None);
    }

    #[test]
    fn chart_x_ticks_add_reliable_round_and_hide_overflowing_edges() {
        let range = 0.0..=300.0;
        let markers = [
            ChartRoundMarker {
                start_seconds: 0.0,
                step: 7,
            },
            ChartRoundMarker {
                start_seconds: 60.0,
                step: 8,
            },
        ];
        assert_eq!(format_chart_x_tick(0.0, &range, &markers), "0秒 · 7 回合");
        assert_eq!(format_chart_x_tick(300.0, &range, &markers), "");
        assert_eq!(format_chart_x_tick(60.0, &range, &markers), "1分 · 8 回合");
        assert_eq!(format_chart_x_tick(30.0, &range, &[]), "30秒");
    }

    #[test]
    fn chart_x_ticks_use_label_width_for_the_edge_guard() {
        let markers = [ChartRoundMarker {
            start_seconds: 0.0,
            step: 123,
        }];
        let range = 0.0..=3_900.0;

        assert_eq!(
            format_chart_x_tick_localized(3_000.0, &range, &markers, Language::Chinese, 600.0,),
            "50分 · 123 回合"
        );
        assert_eq!(
            format_chart_x_tick_localized(3_800.0, &range, &markers, Language::Chinese, 600.0,),
            ""
        );
    }

    #[test]
    fn chart_round_labels_advance_with_recorded_combat_epochs() {
        let history = [
            DpsHistoryPoint {
                elapsed_seconds: 0,
                dps: 0,
                combat_round_epoch: 0,
                estimated_step: None,
            },
            DpsHistoryPoint {
                elapsed_seconds: 10,
                dps: 100,
                combat_round_epoch: 3,
                estimated_step: None,
            },
            DpsHistoryPoint {
                elapsed_seconds: 20,
                dps: 0,
                combat_round_epoch: 0,
                estimated_step: None,
            },
            DpsHistoryPoint {
                elapsed_seconds: 30,
                dps: 200,
                combat_round_epoch: 4,
                estimated_step: Some(9),
            },
        ];

        let markers = chart_round_markers(&history, Some((4, 9)));
        assert_eq!(
            markers,
            vec![
                ChartRoundMarker {
                    start_seconds: 10.0,
                    step: 8,
                },
                ChartRoundMarker {
                    start_seconds: 30.0,
                    step: 9,
                },
            ]
        );
        assert_eq!(chart_round_at(9.0, &markers), None);
        assert_eq!(chart_round_at(10.0, &markers), Some(8));
        assert_eq!(chart_round_at(29.0, &markers), Some(8));
        assert_eq!(chart_round_at(30.0, &markers), Some(9));
    }

    #[test]
    fn one_dps_point_survives_chart_processing_without_a_step_estimate() {
        let raw = [[12.0, 180.0]];
        let reduced = downsample_dps_trend(&raw, 600);
        let smooth = smooth_chart_points(&reduced, 4);

        assert_eq!(smooth.len(), 1);
        assert_eq!(chart_point_at_x(&smooth, 12.0), Some(smooth[0]));
        assert_eq!(format_chart_x_tick(60.0, &(0.0..=300.0), &[]), "1分");
    }

    #[test]
    fn chart_trend_downsampling_preserves_time_order_and_range() {
        let points = (0..1_000)
            .map(|second| {
                let dps = if second == 537 {
                    1_234.0
                } else {
                    50.0 + (second as f64 / 25.0).sin() * 20.0
                };
                [second as f64, dps]
            })
            .collect::<Vec<_>>();
        let reduced = downsample_dps_trend(&points, 100);

        assert_eq!(reduced.first(), points.first());
        assert_eq!(reduced.last(), points.last());
        assert!(reduced.len() <= 100);
        assert!(reduced.windows(2).all(|pair| pair[0][0] < pair[1][0]));
    }

    #[test]
    fn smooth_chart_keeps_endpoints_and_never_invents_negative_dps() {
        let points = [[0.0, 0.0], [1.0, 100.0], [2.0, 0.0]];
        let smooth = smooth_chart_points(&points, 4);
        assert_eq!(smooth.first(), points.first());
        assert_eq!(smooth.last(), points.last());
        assert!(smooth.iter().all(|point| point[1] >= 0.0));
    }

    #[test]
    fn template_draft_state_tracks_content_name_and_preset_changes() {
        let applied = AppConfig::default();
        assert!(!message_template_draft_changed(&applied, &applied));
        assert!(!report_template_draft_changed(&applied, &applied));

        let mut content = applied.clone();
        content.message_template.push_str("\nchanged");
        assert!(message_template_draft_changed(&content, &applied));

        let mut name = applied.clone();
        name.round_report_template_preset_names[0].push_str(" changed");
        assert!(report_template_draft_changed(&name, &applied));

        let mut selected = applied.clone();
        selected.select_message_template_preset(1);
        assert!(message_template_draft_changed(&selected, &applied));
        selected.select_message_template_preset(0);
        assert!(!message_template_draft_changed(&selected, &applied));
    }

    #[test]
    fn template_textarea_grows_with_content_and_stops_at_its_maximum() {
        egui::__run_test_ui(|ui| {
            ui.set_width(320.0);

            let mut short = "one line".to_owned();
            let short_top = ui.cursor().top();
            Textarea::new(&mut short)
                .desired_width(320.0)
                .min_height(64.0)
                .auto_resize()
                .max_height(160.0)
                .show(ui);
            let short_height = ui.cursor().top() - short_top;

            let mut long = (0..20)
                .map(|index| format!("template line {index}"))
                .collect::<Vec<_>>()
                .join("\n");
            let long_top = ui.cursor().top();
            Textarea::new(&mut long)
                .desired_width(320.0)
                .min_height(64.0)
                .auto_resize()
                .max_height(160.0)
                .show(ui);
            let long_height = ui.cursor().top() - long_top;

            assert!(long_height > short_height);
            assert!(long_height <= 168.5);
        });
    }

    #[test]
    fn preview_text_centers_glyphs_inside_a_compact_line_box() {
        let job = preview_text_layout_job("first line\nsecond line");
        let format = &job.sections[0].format;

        assert_eq!(format.line_height, Some(20.0));
        assert_eq!(format.valign, egui::Align::Center);
    }

    #[test]
    fn preset_tabs_use_names_with_safe_empty_and_long_fallbacks() {
        let labels = preset_tab_labels(
            &[
                "日常".to_owned(),
                "  ".to_owned(),
                "这是一个非常非常非常长的预设名称".to_owned(),
            ],
            Language::Chinese,
        );

        assert_eq!(labels[0], "日常");
        assert_eq!(labels[1], "预设 2");
        assert!(labels[2].ends_with('…'));
    }

    #[test]
    fn english_default_report_tab_names_are_not_truncated() {
        let config = AppConfig::defaults_for_language(Language::English);

        assert_eq!(
            preset_tab_labels(
                &config.round_report_template_preset_names,
                Language::English
            ),
            ["DPS Report", "Tank Report", "Backup Report"]
        );
    }

    #[test]
    fn preset_reset_button_is_vertically_centered_with_tabs() {
        egui::__run_test_ui(|ui| {
            let mut selected = 0;
            let mut tabs_rect = egui::Rect::NOTHING;
            let mut reset_rect = egui::Rect::NOTHING;

            preset_controls_row(ui, |ui| {
                tabs_rect = ToggleGroup::new(vec![
                    "Preset 1".to_owned(),
                    "Preset 2".to_owned(),
                    "Preset 3".to_owned(),
                ])
                .variant(ToggleVariant::Outline)
                .size(ComponentSize::Xs)
                .show(ui, &mut selected)
                .rect;
                ui.add_space(6.0);
                reset_rect = ShadcnButton::new("Reset preset")
                    .icon(LucideIcon::RotateCcw)
                    .variant(ButtonVariant::Ghost)
                    .size(ComponentSize::Xs)
                    .height(TEMPLATE_PRESET_TAB_ROW_HEIGHT)
                    .show(ui)
                    .rect;
            });

            assert!(
                (tabs_rect.center().y - reset_rect.center().y).abs() <= 0.5,
                "tabs {tabs_rect:?}, reset {reset_rect:?}"
            );
        });
    }

    #[test]
    fn preview_and_long_logs_stay_inside_their_cards() {
        egui::__run_test_ui(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(420.0, 480.0),
                egui::Layout::top_down(egui::Align::LEFT),
                |ui| {
                    ui.set_width(420.0);
                    let right_edge = ui.max_rect().right();
                    let preview = preview_panel(ui, |ui| {
                        preview_text(ui, "DPS: -\nAVG DPS: -\nBOSS LOCK: -");
                    });
                    assert!((preview.rect.right() - right_edge).abs() <= 0.5);

                    let mut selected = 0;
                    let tabs = ToggleGroup::new(vec![
                        "Normal".to_owned(),
                        "Waiting".to_owned(),
                        "Report".to_owned(),
                    ])
                    .size(ComponentSize::Xs)
                    .show(ui, &mut selected);
                    assert!(tabs.rect.height() <= 28.5);

                    let log = LogRow {
                        time: "12:34:56".to_owned(),
                        level: EventLevel::Warning,
                        message: "A long diagnostic message\nthat wraps onto several lines\nwithout pushing metadata\nor the border outside its card."
                            .to_owned(),
                        repeats: 3,
                    };
                    let log_item = log_line(ui, &log, Language::English);
                    assert!(
                        log_item.rect.right() <= right_edge + 0.5,
                        "log item rect {:?}, expected right edge {right_edge}",
                        log_item.rect
                    );
                    assert!(log_item.rect.height() >= 70.0);
                },
            );
        });
    }

    #[test]
    fn previous_report_uses_readable_responsive_columns() {
        assert_eq!(report_stat_column_count(900.0, 4), 4);
        assert_eq!(report_stat_column_count(900.0, 3), 3);
        assert_eq!(report_stat_column_count(620.0, 4), 3);
        assert_eq!(report_stat_column_count(420.0, 4), 2);
        assert_eq!(report_stat_column_count(140.0, 4), 1);

        egui::__run_test_ui(|ui| {
            ui.set_width(420.0);
            let items = [
                ReportStatItem {
                    label: "Effective DPS growth rate",
                    value: "123.4%".to_owned(),
                    color: SETTINGS_SUCCESS,
                },
                ReportStatItem {
                    label: "Damage taken",
                    value: "18446744073709551615".to_owned(),
                    color: SETTINGS_DANGER,
                },
                ReportStatItem {
                    label: "Longest standstill",
                    value: "59min 59s".to_owned(),
                    color: SETTINGS_WARNING,
                },
            ];
            let right_edge = ui.max_rect().right();
            report_stat_group(ui, &items, 3);
            assert!(ui.min_rect().right() <= right_edge + 0.5);
        });
    }

    #[test]
    fn heart_rate_auxiliary_panel_stays_inside_narrow_english_layout() {
        egui::__run_test_ui(|ui| {
            ui.set_width(280.0);
            let right_edge = ui.max_rect().right();
            let mut enabled = false;
            let mut clipboard = None;
            let mut toast_state = ToastState::new();
            let response = ui
                .vertical(|ui| {
                    heart_rate_auxiliary_panel(
                        ui,
                        &mut enabled,
                        &mut clipboard,
                        &mut toast_state,
                        Language::English,
                        false,
                    );
                })
                .response;

            assert!(response.rect.right() <= right_edge + 0.5);
        });
    }

    #[test]
    fn heart_rate_guide_opens_the_published_document() {
        let context = egui::Context::default();
        let output = context.run(egui::RawInput::default(), |context| {
            open_heart_rate_guide(context);
        });

        assert!(output.platform_output.commands.iter().any(|command| {
            matches!(
                command,
                egui::OutputCommand::OpenUrl(open_url)
                    if open_url.url == HEART_RATE_GUIDE_URL && open_url.new_tab
            )
        }));
    }

    #[test]
    fn heart_rate_help_button_is_vertically_centered_with_title() {
        egui::__run_test_ui(|ui| {
            let (title, help) = heart_rate_title_row(ui, Language::English);

            assert!(
                (title.rect.center().y - help.rect.center().y).abs() <= 0.5,
                "title {:?}, help {:?}",
                title.rect,
                help.rect
            );
        });
    }

    #[test]
    fn variable_rows_keep_a_fixed_button_column_when_wrapping() {
        let variables = [
            VariableHelp {
                role: "数值",
                name: "latest_dps",
                description: "test",
                enabled: true,
            },
            VariableHelp {
                role: "条件",
                name: "has_round_report_effective_dps",
                description: "test",
                enabled: true,
            },
            VariableHelp {
                role: "条件",
                name: "has_round_longest_standstill",
                description: "test",
                enabled: true,
            },
        ];
        let group = VariableHelpGroup {
            title: "测试变量组",
            color: SETTINGS_ACCENT,
            variables: variables.into(),
        };

        let ctx = egui::Context::default();
        let mut first = None;
        let mut second = None;
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(440.0, 300.0),
                )),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let mut clipboard = None;
                    let mut toast_state = ToastState::new();
                    first = Some(variable_help_group_row(
                        ui,
                        0,
                        &group,
                        &mut clipboard,
                        &mut toast_state,
                        Language::Chinese,
                    ));
                    second = Some(variable_help_group_row(
                        ui,
                        1,
                        &group,
                        &mut clipboard,
                        &mut toast_state,
                        Language::Chinese,
                    ));
                });
            },
        );
        let first = first.expect("first row should render");
        let second = second.expect("second row should render");

        assert!((first.buttons_start_x - second.buttons_start_x).abs() <= 0.5);
        assert!(first.label_rect.right() < first.buttons_start_x);
        assert!(first.button_rects.len() >= 3);
        assert!(
            (first.label_text_rect.center().y - first.button_rects[0].center().y).abs() <= 0.5,
            "label {:?} should align with the first button row {:?}",
            first.label_text_rect,
            first.button_rects[0]
        );
        assert!(
            first
                .button_rects
                .iter()
                .all(|rect| rect.left() >= first.buttons_start_x - 0.5)
        );

        let mut previous_top = None;
        let mut wrapped_lines = 0;
        for rect in &first.button_rects {
            let begins_new_line = previous_top
                .map(|top: f32| (rect.top() - top).abs() > 0.5)
                .unwrap_or(true);
            if begins_new_line {
                wrapped_lines += 1;
                assert!(
                    (rect.left() - first.buttons_start_x).abs() <= 0.5,
                    "wrapped button started at {}, expected {}",
                    rect.left(),
                    first.buttons_start_x
                );
                previous_top = Some(rect.top());
            }
        }
        assert!(
            wrapped_lines >= 2,
            "test data must exercise wrapping: {:?}",
            first.button_rects
        );
    }

    #[test]
    fn overlay_report_numbers_stay_compact() {
        assert_eq!(compact_u64(9_999, Language::Chinese), "9999");
        assert_eq!(compact_u64(10_000, Language::Chinese), "1万");
        assert_eq!(compact_u64(12_800, Language::Chinese), "1.3万");
        assert_eq!(compact_u64(123_456_789, Language::Chinese), "1.2亿");
        assert_eq!(compact_f64(38.0, Language::Chinese), "38.0");
        assert_eq!(compact_f64(12_480.0, Language::Chinese), "1.2万");
    }

    #[test]
    fn overlay_status_dot_keeps_its_original_fixed_color() {
        assert_eq!(
            OVERLAY_STATUS_DOT_COLOR,
            egui::Color32::from_rgb(83, 211, 225)
        );
    }

    #[test]
    fn familiar_round_report_highlights_stay_yellow() {
        assert_eq!(METRIC_BEST_DPS, SETTINGS_WARNING);
        assert_eq!(METRIC_STANDSTILL, SETTINGS_WARNING);
    }

    #[test]
    fn template_variables_are_colored_by_category_with_darker_has_flags() {
        let live = localized_variable_groups(
            ecliptica_data_analyzer::i18n::LIVE_VARIABLE_GROUPS,
            Language::Chinese,
            true,
        );
        let report = localized_variable_groups(
            ecliptica_data_analyzer::i18n::REPORT_VARIABLE_GROUPS,
            Language::Chinese,
            true,
        );

        for groups in [&live, &report] {
            for pair in groups.windows(2) {
                assert_ne!(pair[0].color, pair[1].color);
            }
            assert!(groups.iter().all(|group| group.color != SETTINGS_WARNING));
        }
        assert_eq!(live[2].color, report[3].color);
        assert_eq!(live[3].color, report[4].color);

        let value_color = variable_chip_color(live[0].color, "latest_dps");
        let flag_color = variable_chip_color(live[0].color, "has_latest_dps");
        assert_eq!(value_color, live[0].color);
        assert_ne!(flag_color, value_color);
        assert!(flag_color.r() < value_color.r());
        assert!(flag_color.g() < value_color.g());
        assert!(flag_color.b() < value_color.b());
    }

    #[test]
    fn regular_overlay_stays_compact_without_losing_expanded_states() {
        assert_eq!(OVERLAY_WIDTH, 340.0);
        assert_eq!(overlay_height(false, false), 204.0);
        assert_eq!(overlay_height(false, true), 252.0);
        assert_eq!(overlay_height(true, false), 252.0);
        assert_eq!(overlay_height(true, true), 300.0);

        // Combat uses four cards; reports use both two- and three-card rows.
        // Each card's outer width must remain strictly inside its egui column.
        for scale in OVERLAY_SCALE_OPTIONS {
            for (columns, base_margin) in [(4, 4.0), (2, 6.0), (3, 6.0)] {
                let horizontal_margin = base_margin * scale;
                let column_width = (OVERLAY_CONTENT_WIDTH * scale
                    - OVERLAY_ITEM_SPACING.x * scale * (columns - 1) as f32)
                    / columns as f32;
                let card_outer_width =
                    overlay_card_inner_width(column_width, horizontal_margin, scale)
                        + horizontal_margin * 2.0;
                assert!(
                    card_outer_width < column_width,
                    "{columns}-column Overlay row would overflow at {scale}x: {card_outer_width} >= {column_width}"
                );
            }
        }
    }

    #[test]
    fn heart_rate_tag_fits_the_existing_overlay_header_row() {
        egui::__run_test_ui(|ui| {
            for scale in OVERLAY_SCALE_OPTIONS {
                ui.set_width(OVERLAY_CONTENT_WIDTH * scale);
                let content_left = ui.max_rect().left();
                let content_right = ui.max_rect().right();
                let (offline, offline_tag) = overlay_header(ui, None, scale, Language::English);
                let (online, online_tag) = overlay_header(ui, Some(999), scale, Language::English);
                let online_tag = online_tag.expect("online heart rate should render a tag");

                assert!(offline_tag.is_none());
                assert!(
                    online_tag.rect.right() <= content_right + 0.5
                        && online_tag.rect.left() >= content_left - 0.5,
                    "heart-rate tag overflowed the Overlay header at {scale}x"
                );
                assert!(
                    online.rect.height() <= offline.rect.height() + 0.5,
                    "heart-rate tag increased the Overlay header height at {scale}x: {} > {}",
                    online.rect.height(),
                    offline.rect.height()
                );
            }
        });
    }

    #[test]
    fn combat_and_report_card_rows_stay_inside_their_columns() {
        egui::__run_test_ui(|ui| {
            for scale in OVERLAY_SCALE_OPTIONS {
                ui.set_width(OVERLAY_CONTENT_WIDTH * scale);
                let spacing = ui.spacing_mut();
                spacing.item_spacing = OVERLAY_ITEM_SPACING * scale;
                spacing.interact_size = egui::vec2(40.0, 18.0) * scale;

                ui.columns(4, |columns| {
                    for (index, column) in columns.iter_mut().enumerate() {
                        let right_edge = column.max_rect().right();
                        let response = overlay_stat(
                            column,
                            ["最新", "有效", "10秒", "承伤"][index],
                            "完整指标名称",
                            "9999",
                            "18446744073709551615",
                            egui::Color32::WHITE,
                            scale,
                            Language::English,
                        );
                        assert!(response.rect.right() <= right_edge + 0.5);
                    }
                });

                ui.columns(2, |columns| {
                    for column in columns {
                        let right_edge = column.max_rect().right();
                        let response = overlay_report_stat(
                            column,
                            "总输出",
                            "1.2亿",
                            "18446744073709551615",
                            egui::Color32::WHITE,
                            scale,
                            Language::English,
                        );
                        assert!(response.rect.right() <= right_edge + 0.5);
                    }
                });

                ui.columns(3, |columns| {
                    for (index, column) in columns.iter_mut().enumerate() {
                        let right_edge = column.max_rect().right();
                        let response = overlay_report_stat(
                            column,
                            ["有效DPS", "10秒爆发", "承伤"][index],
                            "1.2亿",
                            "18446744073709551615",
                            egui::Color32::WHITE,
                            scale,
                            Language::English,
                        );
                        assert!(response.rect.right() <= right_edge + 0.5);
                    }
                });
            }
        });
    }

    #[test]
    fn boss_lock_row_stays_centered_and_right_aligned_at_every_scale() {
        for scale in OVERLAY_SCALE_OPTIONS {
            egui::__run_test_ui(|ui| {
                ui.set_width(OVERLAY_CONTENT_WIDTH * scale);
                let spacing = ui.spacing_mut();
                spacing.item_spacing = OVERLAY_ITEM_SPACING * scale;
                spacing.interact_size = egui::vec2(40.0, 18.0) * scale;

                let (_card, row, label, value) =
                    lock_card(ui, "Player 123", false, scale, Language::English);
                assert!(
                    (label.rect.center().y - value.rect.center().y).abs() <= 0.5,
                    "Boss Lock labels are not vertically centered at {scale}x: {:?} vs {:?}",
                    label.rect,
                    value.rect
                );
                assert!(
                    (row.rect.right() - value.rect.right()).abs() <= 0.5,
                    "Boss Lock value is not right aligned at {scale}x: {:?} vs {:?}",
                    row.rect,
                    value.rect
                );
            });
        }
    }

    #[test]
    fn chinese_boss_lock_label_uses_one_font_for_latin_and_han_glyphs() {
        let font = overlay_lock_label_font(Language::Chinese, 1.0).unwrap();
        assert_eq!(font.family, egui::FontFamily::Name(CJK_FONT_FAMILY.into()));
        assert_eq!(font.size, 10.0);
        assert!(overlay_lock_label_font(Language::English, 1.0).is_none());
    }

    #[test]
    fn player_name_fonts_include_extended_text_and_symbol_fallbacks() {
        let mut fonts = egui::FontDefinitions::default();
        append_font_fallback(&mut fonts, CJK_FONT_FAMILY);
        append_font_fallback(&mut fonts, EXTENDED_TEXT_FONT_FAMILY);
        append_font_fallback(&mut fonts, SYMBOL_FONT_FAMILY);

        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            let fallbacks = &fonts.families[&family];
            assert!(fallbacks.ends_with(&[
                CJK_FONT_FAMILY.to_owned(),
                EXTENDED_TEXT_FONT_FAMILY.to_owned(),
                SYMBOL_FONT_FAMILY.to_owned(),
            ]));
        }
    }
}
