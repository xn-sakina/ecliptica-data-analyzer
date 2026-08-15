use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::Duration,
};

#[cfg(unix)]
use std::fs::File;

use anyhow::{Context, Result, bail};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;

use crate::APP_ID;
use crate::i18n::Language;

pub const CONFIG_VERSION: u32 = 21;
pub const MESSAGE_TEMPLATE_PRESET_COUNT: usize = 3;
pub const ROUND_REPORT_TEMPLATE_PRESET_COUNT: usize = 3;
pub const TEMPLATE_PRESET_NAME_MAX_CHARS: usize = 24;
const VERSION_5_DEFAULT_TEMPLATE: &str = "{{#if has_latest_dps}}\nDPS: {{latest_dps}}\n30S DPS: {{ave_dps}}\nROUND DPS: {{round_ave_dps}}\n{{/if}}\n{{#if has_max_dps}}\nMAX DPS: {{max_dps}}\n{{/if}}\n{{#if has_boss_lock}}\nBOSS LOCK: {{boss_lock}}\n{{/if}}";
const VERSION_5_DEFAULT_ROUND_REPORT_TEMPLATE: &str = "【回合战报】\n用时 {{round_duration}}｜总输出 {{round_total_damage}}\n峰值 {{round_max_dps}} DPS｜平均 {{round_report_ave_dps}} DPS";
const VERSION_10_DEFAULT_ROUND_REPORT_TEMPLATE: &str = "【回合战报】\n用时 {{round_duration}}｜输出 {{round_total_damage}}｜承伤 {{round_report_damage_taken}}\n有效 {{round_report_effective_dps}} DPS｜10秒爆发 {{round_report_burst_10s}}";
const VERSION_13_DEFAULT_TEMPLATE: &str = "{{#if has_latest_dps}}\nDPS: {{latest_dps}}\n有效 DPS: {{round_effective_dps}}\n10S 爆发: {{round_burst_10s}}\n{{/if}}\n{{#if has_round_damage_taken}}\n回合承伤: {{round_damage_taken}}\n{{/if}}\n{{#if boss_lock}}\nBOSS LOCK: {{boss_lock}}\n{{/if}}";
const VERSION_13_DEFAULT_ROUND_REPORT_TEMPLATE: &str = "【回合战报】\n用时 {{round_duration}}｜输出 {{round_total_damage}}｜承伤 {{round_report_damage_taken}}\n有效 {{round_report_effective_dps}} DPS｜10秒爆发 {{round_report_burst_10s}}\n站桩 {{round_longest_standstill}}";
const VERSION_15_DEFAULT_ROUND_REPORT_TEMPLATE: &str = "【回合战报】\n用时 {{round_duration}}｜我打了 {{round_total_damage}}\n平均 {{round_report_effective_dps}} DPS｜最高 {{round_max_dps}} DPS";
const VERSION_15_DEFAULT_ROUND_REPORT_TEMPLATE_PRESET_2: &str = "【回合战报】\n用时 {{round_duration}}｜被草 {{round_report_damage_taken}}\n最长站桩 {{round_longest_standstill}}s";
const VERSION_16_DEFAULT_TEMPLATE_ENGLISH: &str = "{{#if is_self_boss_locked}}\n【Boss is targeting me — help!】\n{{/if}}\n{{#if rapid_damage_danger}}\n【Taking heavy damage — help!】\n{{/if}}\n{{#if no_wasd_for_10s}}\n【I haven't moved for 10 seconds】\n{{/if}}\n{{#if no_dps_for_10s}}\n【No damage for 10 seconds】\n{{/if}}\n{{#if has_latest_dps}}\nDPS: {{latest_dps}}\n{{/if}}\n";
const VERSION_16_DEFAULT_TEMPLATE_PRESET_2_ENGLISH: &str = "{{#if is_self_boss_locked}}\n【Boss is targeting me — attack it!】\n{{/if}}\n{{#if no_wasd_for_10s}}\n【I haven't moved for 10 seconds】\n{{/if}}\n{{#if no_dps_for_10s}}\n【No damage for 10 seconds】\n{{/if}}\n{{#if has_round_damage_taken}}\nDamage taken: {{round_damage_taken}}\n{{/if}}\n";
const VERSION_17_DEFAULT_ROUND_REPORT_TEMPLATE_PRESET_2: &str = "【回合战报】\n用时 {{round_duration}}｜被草 {{round_report_damage_taken}}\n最长站桩 {{round_longest_standstill}}s\n{{#if has_step_estimate}}\n预计距 Jim 还有 {{until_boss_step}} 回合\n{{/if}}\n";
const VERSION_13_DEFAULT_ALERT_VOLUME: f32 = 0.35;
pub const DEFAULT_TEMPLATE: &str = include_str!("../resources/presets/zh/combat1.txt");
pub const DEFAULT_TEMPLATE_PRESET_2: &str = include_str!("../resources/presets/zh/combat2.txt");
pub const DEFAULT_ROUND_REPORT_TEMPLATE: &str = include_str!("../resources/presets/zh/report1.txt");
pub const DEFAULT_ROUND_REPORT_TEMPLATE_PRESET_2: &str =
    include_str!("../resources/presets/zh/report2.txt");
const DEFAULT_TEMPLATE_ENGLISH: &str = include_str!("../resources/presets/en/combat1.txt");
const DEFAULT_TEMPLATE_PRESET_2_ENGLISH: &str = include_str!("../resources/presets/en/combat2.txt");
const DEFAULT_ROUND_REPORT_TEMPLATE_ENGLISH: &str =
    include_str!("../resources/presets/en/report1.txt");
const DEFAULT_ROUND_REPORT_TEMPLATE_PRESET_2_ENGLISH: &str =
    include_str!("../resources/presets/en/report2.txt");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SendInterval {
    One,
    OnePointFive,
    Two,
    Three,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AlertSoundStyle {
    Soft,
    Crisp,
    Prominent,
}

impl AlertSoundStyle {
    pub const ALL: [Self; 3] = [Self::Soft, Self::Crisp, Self::Prominent];

    pub fn display_label(self, language: Language) -> &'static str {
        match self {
            Self::Soft => crate::i18n::text::SOUND_SOFT.get(language),
            Self::Crisp => crate::i18n::text::SOUND_CRISP.get(language),
            Self::Prominent => crate::i18n::text::SOUND_PROMINENT.get(language),
        }
    }
}

impl SendInterval {
    pub fn duration(self) -> Duration {
        match self {
            Self::One => Duration::from_secs(1),
            Self::OnePointFive => Duration::from_millis(1_500),
            Self::Two => Duration::from_secs(2),
            Self::Three => Duration::from_secs(3),
        }
    }

    pub fn seconds_label(self) -> &'static str {
        match self {
            Self::One => "1",
            Self::OnePointFive => "1.5",
            Self::Two => "2",
            Self::Three => "3",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub version: u32,
    pub language: Language,
    pub send_interval: SendInterval,
    pub message_template: String,
    pub message_template_presets: [String; MESSAGE_TEMPLATE_PRESET_COUNT],
    pub message_template_preset_names: [String; MESSAGE_TEMPLATE_PRESET_COUNT],
    pub active_message_template_preset: usize,
    pub round_report_template: String,
    pub round_report_template_presets: [String; ROUND_REPORT_TEMPLATE_PRESET_COUNT],
    pub round_report_template_preset_names: [String; ROUND_REPORT_TEMPLATE_PRESET_COUNT],
    pub active_round_report_template_preset: usize,
    pub display_name: String,
    pub alert_volume: f32,
    pub locked_sound_style: AlertSoundStyle,
    pub unlocked_sound_style: AlertSoundStyle,
    pub overlay_x: f32,
    pub overlay_y: f32,
    pub overlay_scale: f32,
    pub overlay_locked: bool,
    pub overlay_mouse_passthrough: bool,
    pub osc_enabled: bool,
    /// Accept updates from a compatible local HTTP heart-rate sender.
    #[serde(default)]
    pub heart_rate_enabled: bool,
    pub osc_address: String,
    pub stale_after_seconds: u64,
    pub log_path_override: Option<PathBuf>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self::defaults_for_language(Language::default())
    }
}

impl AppConfig {
    pub fn defaults_for_language(language: Language) -> Self {
        let message_template_presets = default_message_template_presets(language);
        let round_report_template_presets = default_round_report_template_presets(language);
        Self {
            version: CONFIG_VERSION,
            language,
            send_interval: SendInterval::One,
            message_template: message_template_presets[0].clone(),
            message_template_presets,
            message_template_preset_names: default_template_preset_names(language),
            active_message_template_preset: 0,
            round_report_template: round_report_template_presets[0].clone(),
            round_report_template_presets,
            round_report_template_preset_names: default_template_preset_names(language),
            active_round_report_template_preset: 0,
            display_name: String::new(),
            alert_volume: 1.0,
            locked_sound_style: AlertSoundStyle::Soft,
            unlocked_sound_style: AlertSoundStyle::Soft,
            // Keep clear of the default left-side macOS Dock during local development.
            overlay_x: if cfg!(target_os = "macos") {
                82.0
            } else {
                18.0
            },
            overlay_y: 140.0,
            overlay_scale: 1.0,
            overlay_locked: true,
            overlay_mouse_passthrough: true,
            osc_enabled: true,
            heart_rate_enabled: false,
            osc_address: "127.0.0.1:9000".to_owned(),
            stale_after_seconds: 10,
            log_path_override: None,
        }
    }

    /// Return a copy using `language` and translate only untouched built-in
    /// preset text/names. Exact matching deliberately treats even whitespace
    /// edits as user customization.
    pub fn with_localized_defaults(&self, language: Language) -> Self {
        let mut localized = self.clone();
        let chinese_messages = default_message_template_presets(Language::Chinese);
        let english_messages = default_message_template_presets(Language::English);
        let target_messages = default_message_template_presets(language);
        let chinese_reports = default_round_report_template_presets(Language::Chinese);
        let english_reports = default_round_report_template_presets(Language::English);
        let target_reports = default_round_report_template_presets(language);
        let chinese_names =
            default_template_preset_names::<MESSAGE_TEMPLATE_PRESET_COUNT>(Language::Chinese);
        let english_names =
            default_template_preset_names::<MESSAGE_TEMPLATE_PRESET_COUNT>(Language::English);
        let target_names = default_template_preset_names::<MESSAGE_TEMPLATE_PRESET_COUNT>(language);

        localize_builtin_value(
            &mut localized.message_template,
            [&chinese_messages[0], &english_messages[0]],
            &target_messages[0],
        );
        localize_builtin_value(
            &mut localized.message_template,
            [&chinese_messages[1], &english_messages[1]],
            &target_messages[1],
        );
        localize_builtin_value(
            &mut localized.round_report_template,
            [&chinese_reports[0], &english_reports[0]],
            &target_reports[0],
        );
        localize_builtin_value(
            &mut localized.round_report_template,
            [&chinese_reports[1], &english_reports[1]],
            &target_reports[1],
        );

        for index in 0..MESSAGE_TEMPLATE_PRESET_COUNT {
            localize_builtin_value(
                &mut localized.message_template_presets[index],
                [&chinese_messages[index], &english_messages[index]],
                &target_messages[index],
            );
            localize_builtin_value(
                &mut localized.message_template_preset_names[index],
                [&chinese_names[index], &english_names[index]],
                &target_names[index],
            );
        }
        for index in 0..ROUND_REPORT_TEMPLATE_PRESET_COUNT {
            localize_builtin_value(
                &mut localized.round_report_template_presets[index],
                [&chinese_reports[index], &english_reports[index]],
                &target_reports[index],
            );
            localize_builtin_value(
                &mut localized.round_report_template_preset_names[index],
                [&chinese_names[index], &english_names[index]],
                &target_names[index],
            );
        }
        localized.language = language;
        localized
    }

    /// Whether the Overlay can currently receive and act on drag input.
    ///
    /// Keep the two serialized legacy flags behind one semantic state so an
    /// unlocked-but-mouse-passthrough configuration is never treated as
    /// draggable.
    pub fn overlay_draggable(&self) -> bool {
        !self.overlay_locked && !self.overlay_mouse_passthrough
    }

    pub fn set_overlay_draggable(&mut self, draggable: bool) {
        self.overlay_locked = !draggable;
        self.overlay_mouse_passthrough = !draggable;
    }

    /// Store edits in the current slot and load another message-template preset.
    pub fn select_message_template_preset(&mut self, preset: usize) -> bool {
        if preset >= MESSAGE_TEMPLATE_PRESET_COUNT {
            return false;
        }
        self.sync_active_message_template_preset();
        self.active_message_template_preset = preset;
        self.message_template = self.message_template_presets[preset].clone();
        true
    }

    /// Keep the legacy/runtime field mirrored into the selected persistent slot.
    pub fn sync_active_message_template_preset(&mut self) {
        if let Some(slot) = self
            .message_template_presets
            .get_mut(self.active_message_template_preset)
        {
            slot.clone_from(&self.message_template);
        }
    }

    /// Restore only the selected message preset's content for the current language.
    pub fn reset_active_message_template_to_default(&mut self) -> bool {
        let preset = self.active_message_template_preset;
        let Some(slot) = self.message_template_presets.get_mut(preset) else {
            return false;
        };
        let default = default_message_template_presets(self.language)[preset].clone();
        slot.clone_from(&default);
        self.message_template = default;
        true
    }

    /// Store edits in the current slot and load another round-report preset.
    pub fn select_round_report_template_preset(&mut self, preset: usize) -> bool {
        if preset >= ROUND_REPORT_TEMPLATE_PRESET_COUNT {
            return false;
        }
        self.sync_active_round_report_template_preset();
        self.active_round_report_template_preset = preset;
        self.round_report_template = self.round_report_template_presets[preset].clone();
        true
    }

    pub fn sync_active_round_report_template_preset(&mut self) {
        if let Some(slot) = self
            .round_report_template_presets
            .get_mut(self.active_round_report_template_preset)
        {
            slot.clone_from(&self.round_report_template);
        }
    }

    /// Restore only the selected report preset's content for the current language.
    pub fn reset_active_round_report_template_to_default(&mut self) -> bool {
        let preset = self.active_round_report_template_preset;
        let Some(slot) = self.round_report_template_presets.get_mut(preset) else {
            return false;
        };
        let default = default_round_report_template_presets(self.language)[preset].clone();
        slot.clone_from(&default);
        self.round_report_template = default;
        true
    }

    pub fn validate(&self) -> Result<()> {
        if self.version > CONFIG_VERSION {
            bail!(
                "{}",
                crate::i18n::format_pattern(
                    crate::i18n::text::CONFIG_VERSION_UNSUPPORTED,
                    self.language,
                    &[
                        ("version", self.version.to_string()),
                        ("supported", CONFIG_VERSION.to_string())
                    ]
                )
            );
        }
        if !(0.0..=1.0).contains(&self.alert_volume) {
            bail!("{}", crate::i18n::text::VOLUME_INVALID.get(self.language));
        }
        if !self.overlay_scale.is_finite() || !(0.5..=3.0).contains(&self.overlay_scale) {
            bail!(
                "{}",
                crate::i18n::text::OVERLAY_SCALE_INVALID.get(self.language)
            );
        }
        if !(2..=300).contains(&self.stale_after_seconds) {
            bail!(
                "{}",
                crate::i18n::text::STALE_TIME_INVALID.get(self.language)
            );
        }
        self.osc_address
            .parse::<std::net::SocketAddr>()
            .context(crate::i18n::text::OSC_ADDRESS_INVALID.get(self.language))?;
        if self.active_message_template_preset >= MESSAGE_TEMPLATE_PRESET_COUNT {
            bail!(
                "{}",
                crate::i18n::format_pattern(
                    crate::i18n::text::MESSAGE_PRESET_RANGE_INVALID,
                    self.language,
                    &[("count", MESSAGE_TEMPLATE_PRESET_COUNT.to_string())]
                )
            );
        }
        validate_template(&self.message_template, self.language)?;
        for (index, template) in self.message_template_presets.iter().enumerate() {
            validate_template(template, self.language).with_context(|| {
                crate::i18n::format_pattern(
                    crate::i18n::text::MESSAGE_PRESET_INVALID,
                    self.language,
                    &[("index", (index + 1).to_string())],
                )
            })?;
        }
        validate_template_preset_names(
            &self.message_template_preset_names,
            crate::i18n::text::MESSAGE_TEMPLATE_KIND.get(self.language),
            self.language,
        )?;
        if self.active_round_report_template_preset >= ROUND_REPORT_TEMPLATE_PRESET_COUNT {
            bail!(
                "{}",
                crate::i18n::format_pattern(
                    crate::i18n::text::REPORT_PRESET_RANGE_INVALID,
                    self.language,
                    &[("count", ROUND_REPORT_TEMPLATE_PRESET_COUNT.to_string())]
                )
            );
        }
        validate_template(&self.round_report_template, self.language)
            .context(crate::i18n::text::REPORT_TEMPLATE_INVALID.get(self.language))?;
        for (index, template) in self.round_report_template_presets.iter().enumerate() {
            validate_template(template, self.language).with_context(|| {
                crate::i18n::format_pattern(
                    crate::i18n::text::REPORT_PRESET_INVALID,
                    self.language,
                    &[("index", (index + 1).to_string())],
                )
            })?;
        }
        validate_template_preset_names(
            &self.round_report_template_preset_names,
            crate::i18n::text::REPORT_TEMPLATE_KIND.get(self.language),
            self.language,
        )?;
        Ok(())
    }

    pub fn migrated(mut self) -> Self {
        if self.version < 6 {
            if self.message_template == VERSION_5_DEFAULT_TEMPLATE {
                self.message_template = DEFAULT_TEMPLATE.to_owned();
            }
            if self.round_report_template == VERSION_5_DEFAULT_ROUND_REPORT_TEMPLATE {
                self.round_report_template = DEFAULT_ROUND_REPORT_TEMPLATE.to_owned();
            }
        }
        if self.version < 7 {
            self.message_template = migrate_average_variable_names(&self.message_template);
            self.round_report_template =
                migrate_average_variable_names(&self.round_report_template);
        }
        if self.version < 8 {
            self.message_template = migrate_redundant_presence_flags(&self.message_template);
            self.round_report_template =
                migrate_redundant_presence_flags(&self.round_report_template);
        }
        if self.version < 9 {
            self.message_template_presets = default_message_template_presets(self.language);
            self.message_template_presets[0] = self.message_template.clone();
            self.active_message_template_preset = 0;
        } else if let Some(template) = self
            .message_template_presets
            .get(self.active_message_template_preset)
        {
            self.message_template.clone_from(template);
        }
        if self.version < 10 {
            self.round_report_template_presets =
                default_round_report_template_presets(self.language);
            self.round_report_template_presets[0] = self.round_report_template.clone();
            self.active_round_report_template_preset = 0;
        } else if let Some(template) = self
            .round_report_template_presets
            .get(self.active_round_report_template_preset)
        {
            self.round_report_template.clone_from(template);
        }
        if self.version < 11 {
            upgrade_builtin_round_report_template(
                &mut self.round_report_template,
                DEFAULT_ROUND_REPORT_TEMPLATE,
            );
            for (template, new_default) in self.round_report_template_presets.iter_mut().zip([
                DEFAULT_ROUND_REPORT_TEMPLATE,
                DEFAULT_ROUND_REPORT_TEMPLATE_PRESET_2,
                DEFAULT_ROUND_REPORT_TEMPLATE,
            ]) {
                upgrade_builtin_round_report_template(template, new_default);
            }
        }
        if self.version < 12 {
            self.message_template_preset_names = default_template_preset_names(self.language);
            self.round_report_template_preset_names = default_template_preset_names(self.language);
        }
        if self.version < 14 {
            upgrade_version_13_builtin_presets(
                &mut self.message_template_presets,
                VERSION_13_DEFAULT_TEMPLATE,
                [
                    DEFAULT_TEMPLATE,
                    DEFAULT_TEMPLATE_PRESET_2,
                    DEFAULT_TEMPLATE,
                ],
            );
            upgrade_version_13_builtin_presets(
                &mut self.round_report_template_presets,
                VERSION_13_DEFAULT_ROUND_REPORT_TEMPLATE,
                [
                    DEFAULT_ROUND_REPORT_TEMPLATE,
                    DEFAULT_ROUND_REPORT_TEMPLATE_PRESET_2,
                    DEFAULT_ROUND_REPORT_TEMPLATE,
                ],
            );
            if let Some(template) = self
                .message_template_presets
                .get(self.active_message_template_preset)
            {
                self.message_template.clone_from(template);
            }
            if let Some(template) = self
                .round_report_template_presets
                .get(self.active_round_report_template_preset)
            {
                self.round_report_template.clone_from(template);
            }
            if self.alert_volume == VERSION_13_DEFAULT_ALERT_VOLUME {
                self.alert_volume = 1.0;
            }
        }
        if self.version < 16 {
            upgrade_version_15_round_report_presets(
                &mut self.round_report_template_presets,
                [
                    DEFAULT_ROUND_REPORT_TEMPLATE,
                    DEFAULT_ROUND_REPORT_TEMPLATE_PRESET_2,
                    DEFAULT_ROUND_REPORT_TEMPLATE,
                ],
            );
            if let Some(template) = self
                .round_report_template_presets
                .get(self.active_round_report_template_preset)
            {
                self.round_report_template.clone_from(template);
            }
        }
        if self.version < 17 {
            if self.message_template == VERSION_16_DEFAULT_TEMPLATE_ENGLISH {
                self.message_template = DEFAULT_TEMPLATE_ENGLISH.to_owned();
            } else if self.message_template == VERSION_16_DEFAULT_TEMPLATE_PRESET_2_ENGLISH {
                self.message_template = DEFAULT_TEMPLATE_PRESET_2_ENGLISH.to_owned();
            }
            for template in &mut self.message_template_presets {
                if template == VERSION_16_DEFAULT_TEMPLATE_ENGLISH {
                    *template = DEFAULT_TEMPLATE_ENGLISH.to_owned();
                } else if template == VERSION_16_DEFAULT_TEMPLATE_PRESET_2_ENGLISH {
                    *template = DEFAULT_TEMPLATE_PRESET_2_ENGLISH.to_owned();
                }
            }
            if let Some(template) = self
                .message_template_presets
                .get(self.active_message_template_preset)
            {
                self.message_template.clone_from(template);
            }
        }
        if self.version < 18 {
            if self.round_report_template == VERSION_17_DEFAULT_ROUND_REPORT_TEMPLATE_PRESET_2 {
                self.round_report_template = DEFAULT_ROUND_REPORT_TEMPLATE_PRESET_2.to_owned();
            }
            for template in &mut self.round_report_template_presets {
                if template == VERSION_17_DEFAULT_ROUND_REPORT_TEMPLATE_PRESET_2 {
                    *template = DEFAULT_ROUND_REPORT_TEMPLATE_PRESET_2.to_owned();
                }
            }
            if let Some(template) = self
                .round_report_template_presets
                .get(self.active_round_report_template_preset)
            {
                self.round_report_template.clone_from(template);
            }
        }
        if self.version < 19 {
            self.message_template = migrate_removed_report_variables(&self.message_template, false);
            self.round_report_template =
                migrate_removed_report_variables(&self.round_report_template, true);
            for template in &mut self.message_template_presets {
                *template = migrate_removed_report_variables(template, false);
            }
            for template in &mut self.round_report_template_presets {
                *template = migrate_removed_report_variables(template, true);
            }
            if let Some(template) = self
                .message_template_presets
                .get(self.active_message_template_preset)
            {
                self.message_template.clone_from(template);
            }
            if let Some(template) = self
                .round_report_template_presets
                .get(self.active_round_report_template_preset)
            {
                self.round_report_template.clone_from(template);
            }
        }
        if self.version < 20 {
            self.message_template =
                migrate_removed_context_variables(&self.message_template, "COMBAT");
            self.round_report_template =
                migrate_removed_context_variables(&self.round_report_template, "LOBBY");
            for template in &mut self.message_template_presets {
                *template = migrate_removed_context_variables(template, "COMBAT");
            }
            for template in &mut self.round_report_template_presets {
                *template = migrate_removed_context_variables(template, "LOBBY");
            }
            if let Some(template) = self
                .message_template_presets
                .get(self.active_message_template_preset)
            {
                self.message_template.clone_from(template);
            }
            if let Some(template) = self
                .round_report_template_presets
                .get(self.active_round_report_template_preset)
            {
                self.round_report_template.clone_from(template);
            }
        }
        self.version = CONFIG_VERSION;
        self.alert_volume = self.alert_volume.clamp(0.0, 1.0);
        if !self.overlay_scale.is_finite() {
            self.overlay_scale = 1.0;
        }
        self.overlay_scale = self.overlay_scale.clamp(0.5, 3.0);
        self.stale_after_seconds = self.stale_after_seconds.clamp(2, 300);
        self
    }
}

fn localize_builtin_value(value: &mut String, builtins: [&String; 2], target: &str) {
    if builtins.into_iter().any(|builtin| value == builtin) {
        *value = target.to_owned();
    }
}

fn default_message_template_presets(language: Language) -> [String; MESSAGE_TEMPLATE_PRESET_COUNT] {
    let (first, second) = match language {
        Language::English => (DEFAULT_TEMPLATE_ENGLISH, DEFAULT_TEMPLATE_PRESET_2_ENGLISH),
        Language::Chinese => (DEFAULT_TEMPLATE, DEFAULT_TEMPLATE_PRESET_2),
    };
    [first.to_owned(), second.to_owned(), first.to_owned()]
}

fn default_round_report_template_presets(
    language: Language,
) -> [String; ROUND_REPORT_TEMPLATE_PRESET_COUNT] {
    let (first, second) = match language {
        Language::English => (
            DEFAULT_ROUND_REPORT_TEMPLATE_ENGLISH,
            DEFAULT_ROUND_REPORT_TEMPLATE_PRESET_2_ENGLISH,
        ),
        Language::Chinese => (
            DEFAULT_ROUND_REPORT_TEMPLATE,
            DEFAULT_ROUND_REPORT_TEMPLATE_PRESET_2,
        ),
    };
    [first.to_owned(), second.to_owned(), first.to_owned()]
}

fn upgrade_version_13_builtin_presets<const N: usize>(
    presets: &mut [String; N],
    old_default: &str,
    new_defaults: [&str; N],
) {
    for (preset, new_default) in presets.iter_mut().zip(new_defaults) {
        if preset == old_default {
            *preset = new_default.to_owned();
        }
    }
}

fn default_template_preset_names<const N: usize>(language: Language) -> [String; N] {
    std::array::from_fn(|index| {
        crate::i18n::format_pattern(
            crate::i18n::text::PRESET_FALLBACK,
            language,
            &[("index", (index + 1).to_string())],
        )
    })
}

fn validate_template_preset_names<const N: usize>(
    names: &[String; N],
    kind: &str,
    language: Language,
) -> Result<()> {
    for (index, name) in names.iter().enumerate() {
        let name = name.trim();
        if name.is_empty() {
            bail!(
                "{}",
                crate::i18n::format_pattern(
                    crate::i18n::text::PRESET_NAME_EMPTY,
                    language,
                    &[
                        ("kind", kind.to_owned()),
                        ("index", (index + 1).to_string())
                    ]
                )
            );
        }
        if name.chars().count() > TEMPLATE_PRESET_NAME_MAX_CHARS {
            bail!(
                "{}",
                crate::i18n::format_pattern(
                    crate::i18n::text::PRESET_NAME_TOO_LONG,
                    language,
                    &[
                        ("kind", kind.to_owned()),
                        ("index", (index + 1).to_string()),
                        ("max", TEMPLATE_PRESET_NAME_MAX_CHARS.to_string())
                    ]
                )
            );
        }
    }
    Ok(())
}

fn migrate_average_variable_names(template: &str) -> String {
    [
        ("has_round_report_ave_dps", "has_round_report_avg_dps"),
        ("round_report_ave_dps", "round_report_avg_dps"),
        ("has_round_ave_dps", "has_round_avg_dps"),
        ("round_ave_dps", "round_avg_dps"),
        ("has_ave_dps", "has_avg_dps"),
        ("ave_dps", "avg_dps"),
    ]
    .into_iter()
    .fold(template.to_owned(), |template, (old, new)| {
        template.replace(old, new)
    })
}

fn migrate_redundant_presence_flags(template: &str) -> String {
    [
        ("has_round_report_damage_taken", "has_round_report"),
        ("has_round_total_damage", "has_round_report"),
        ("has_no_wasd_for_10s", "true"),
        ("has_boss_lock", "boss_lock"),
        ("has_boss", "boss"),
    ]
    .into_iter()
    .fold(template.to_owned(), |template, (old, new)| {
        template.replace(old, new)
    })
}

fn migrate_removed_report_variables(template: &str, report_context: bool) -> String {
    // Template selection already supplies the report presence: always false
    // for combat templates and always true for report templates. Word
    // boundaries avoid touching useful variables such as
    // `has_round_report_effective_dps`.
    let without_report_flag = regex::Regex::new(r"\bhas_round_report\b")
        .expect("built-in report flag migration regex must compile")
        .replace_all(template, report_context.to_string());
    // These values have deliberately been removed. Strip their simplest
    // interpolation form so upgraded custom templates remain valid.
    let without_interpolations =
        regex::Regex::new(r"\{\{\s*(?:has_round_combat_duration|round_combat_duration)\s*\}\}")
            .expect("built-in combat-duration migration regex must compile")
            .replace_all(&without_report_flag, "")
            .into_owned();
    regex::Regex::new(r"\b(?:has_round_combat_duration|round_combat_duration)\b")
        .expect("built-in removed-variable migration regex must compile")
        .replace_all(&without_interpolations, "false")
        .into_owned()
}

fn migrate_removed_context_variables(template: &str, phase: &str) -> String {
    let expression = regex::Regex::new(r"\{\{[^}]*\}\}")
        .expect("built-in template-expression migration regex must compile");
    let phase_reference =
        regex::Regex::new(r"\bphase\b").expect("built-in phase migration regex must compile");
    let status_reference =
        regex::Regex::new(r"\bstatus\b").expect("built-in status migration regex must compile");
    let waiting_reference = regex::Regex::new(r"\bwaiting_for_next_round\b")
        .expect("built-in waiting migration regex must compile");

    expression
        .replace_all(template, |captures: &regex::Captures<'_>| {
            let source = captures.get(0).expect("whole expression exists").as_str();
            if regex::Regex::new(r"^\{\{\s*(?:phase|status|waiting_for_next_round)\s*\}\}$")
                .expect("built-in direct interpolation regex must compile")
                .is_match(source)
            {
                return match source
                    .trim_matches(|character| character == '{' || character == '}')
                    .trim()
                {
                    "phase" => phase.to_owned(),
                    "status" => "LIVE".to_owned(),
                    _ => String::new(),
                };
            }
            let migrated = phase_reference.replace_all(source, format!(r#""{phase}""#));
            let migrated = status_reference.replace_all(&migrated, r#""LIVE""#);
            waiting_reference
                .replace_all(&migrated, "false")
                .into_owned()
        })
        .into_owned()
}

fn upgrade_builtin_round_report_template(template: &mut String, new_default: &str) {
    if template == VERSION_10_DEFAULT_ROUND_REPORT_TEMPLATE {
        *template = new_default.to_owned();
    }
}

fn upgrade_version_15_round_report_presets(
    presets: &mut [String; ROUND_REPORT_TEMPLATE_PRESET_COUNT],
    replacements: [&str; ROUND_REPORT_TEMPLATE_PRESET_COUNT],
) {
    for (template, replacement) in presets.iter_mut().zip(replacements) {
        if template.trim_end() == VERSION_15_DEFAULT_ROUND_REPORT_TEMPLATE
            || template.trim_end() == VERSION_15_DEFAULT_ROUND_REPORT_TEMPLATE_PRESET_2
        {
            *template = replacement.to_owned();
        }
    }
}

pub fn validate_template(source: &str, language: Language) -> Result<()> {
    let mut handlebars = handlebars::Handlebars::new();
    handlebars.set_strict_mode(true);
    handlebars
        .register_template_string("message", source)
        .context(crate::i18n::text::TEMPLATE_SYNTAX_ERROR.get(language))?;
    let values = serde_json::json!({
        "latest_dps": "-",
        "avg_dps": "-",
        "round_avg_dps": "-",
        "round_effective_dps": "-",
        "round_burst_10s": "-",
        "round_damage_taken": "0",
        "max_dps": "-",
        "boss_lock": "",
        "boss": "",
        "heart_rate": "-",
        "has_heart_rate": false,
        "has_latest_dps": false,
        "has_avg_dps": false,
        "has_round_avg_dps": false,
        "has_round_effective_dps": false,
        "has_round_burst_10s": false,
        "has_round_damage_taken": false,
        "has_max_dps": false,
        "is_self_boss_locked": false,
        "rapid_damage_danger": false,
        "no_dps_for_10s": false,
        "no_wasd_for_10s": false,
        "has_round_duration": false,
        "has_round_report_avg_dps": false,
        "has_round_max_dps": false,
        "has_round_report_effective_dps": false,
        "has_round_report_burst_10s": false,
        "has_dps_growth_rate": false,
        "has_round_dps_growth_rate": false,
        "has_round_longest_standstill": false,
        "has_step_estimate": false,
        "current_step": "-",
        "until_boss_step": "-",
        "round_duration": "-",
        "round_total_damage": "-",
        "round_report_avg_dps": "-",
        "round_max_dps": "-",
        "round_report_effective_dps": "-",
        "round_report_burst_10s": "-",
        "dps_growth_rate": "0",
        "round_dps_growth_rate": "0",
        "round_report_damage_taken": "-",
        "round_longest_standstill": "-"
    });
    handlebars
        .render("message", &values)
        .context(crate::i18n::text::TEMPLATE_UNKNOWN_VARIABLE.get(language))?;
    Ok(())
}

pub fn config_dir() -> Result<PathBuf> {
    let language = Language::system_default();
    ProjectDirs::from("com", "Ecliptica", APP_ID)
        .map(|dirs| dirs.config_dir().to_path_buf())
        .context(crate::i18n::text::CONFIG_DIR_UNAVAILABLE.get(language))
}

pub fn config_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("config.json"))
}

pub fn load_or_recover() -> Result<(AppConfig, Option<String>)> {
    let language = Language::system_default();
    let path = config_path()?;
    if !path.exists() {
        return Ok((AppConfig::default(), None));
    }

    match fs::read(&path)
        .with_context(|| {
            crate::i18n::format_pattern(
                crate::i18n::text::CONFIG_READ_FAILED,
                language,
                &[("path", path.display().to_string())],
            )
        })
        .and_then(|bytes| {
            serde_json::from_slice::<AppConfig>(&bytes)
                .context(crate::i18n::text::CONFIG_JSON_CORRUPT.get(language))
        })
        .map(AppConfig::migrated)
        .and_then(|config| {
            config.validate()?;
            Ok(config)
        }) {
        Ok(config) => Ok((config, None)),
        Err(error) => {
            let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
            let backup = path.with_file_name(format!("config.corrupt-{stamp}.json"));
            fs::rename(&path, &backup).with_context(|| {
                crate::i18n::format_pattern(
                    crate::i18n::text::CONFIG_BACKUP_FAILED,
                    language,
                    &[("path", backup.display().to_string())],
                )
            })?;
            Ok((
                AppConfig::defaults_for_language(language),
                Some(format!(
                    "{} ({error:#})",
                    crate::i18n::format_pattern(
                        crate::i18n::text::CONFIG_RECOVERED,
                        language,
                        &[("path", backup.display().to_string())],
                    )
                )),
            ))
        }
    }
}

pub fn save_atomic(config: &AppConfig) -> Result<()> {
    config.validate()?;
    let dir = config_dir()?;
    fs::create_dir_all(&dir).with_context(|| {
        crate::i18n::format_pattern(
            crate::i18n::text::CONFIG_DIR_CREATE_FAILED,
            config.language,
            &[("path", dir.display().to_string())],
        )
    })?;
    let path = dir.join("config.json");
    let mut temp = NamedTempFile::new_in(&dir)
        .context(crate::i18n::text::CONFIG_TEMP_CREATE_FAILED.get(config.language))?;
    serde_json::to_writer_pretty(&mut temp, config)
        .context(crate::i18n::text::CONFIG_SERIALIZE_FAILED.get(config.language))?;
    temp.write_all(b"\n")?;
    temp.as_file()
        .sync_all()
        .context(crate::i18n::text::CONFIG_TEMP_SYNC_FAILED.get(config.language))?;
    temp.persist(&path)
        .map_err(|error| error.error)
        .with_context(|| {
            crate::i18n::format_pattern(
                crate::i18n::text::CONFIG_REPLACE_FAILED,
                config.language,
                &[("path", path.display().to_string())],
            )
        })?;
    sync_parent(&dir)?;
    Ok(())
}

fn sync_parent(_dir: &Path) -> Result<()> {
    #[cfg(unix)]
    File::open(_dir)?.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_point_five_second_send_interval_is_exact_and_persistent() {
        assert_eq!(
            SendInterval::OnePointFive.duration(),
            Duration::from_millis(1_500)
        );
        assert_eq!(SendInterval::OnePointFive.seconds_label(), "1.5");

        let json = serde_json::to_string(&SendInterval::OnePointFive).unwrap();
        assert_eq!(json, "\"OnePointFive\"");
        assert_eq!(
            serde_json::from_str::<SendInterval>(&json).unwrap(),
            SendInterval::OnePointFive
        );
    }

    #[test]
    fn language_is_persisted_and_old_configs_receive_a_default() {
        let config = AppConfig {
            language: Language::Chinese,
            ..AppConfig::default()
        };
        let restored =
            serde_json::from_value::<AppConfig>(serde_json::to_value(config).unwrap()).unwrap();
        assert_eq!(restored.language, Language::Chinese);

        let mut old_value = serde_json::to_value(AppConfig::default()).unwrap();
        old_value.as_object_mut().unwrap().remove("language");
        assert!(serde_json::from_value::<AppConfig>(old_value).is_ok());
    }

    #[test]
    fn old_configs_default_to_heart_rate_disabled() {
        let mut value = serde_json::to_value(AppConfig::default()).unwrap();
        let object = value.as_object_mut().unwrap();
        object.insert("version".to_owned(), serde_json::json!(20));
        object.remove("heart_rate_enabled");
        let migrated = serde_json::from_value::<AppConfig>(value)
            .unwrap()
            .migrated();
        assert_eq!(migrated.version, CONFIG_VERSION);
        assert!(!migrated.heart_rate_enabled);
    }

    #[test]
    fn alert_sound_styles_are_selected_and_persisted_independently() {
        let config = AppConfig {
            locked_sound_style: AlertSoundStyle::Crisp,
            unlocked_sound_style: AlertSoundStyle::Prominent,
            ..AppConfig::default()
        };

        let restored = serde_json::from_value::<AppConfig>(serde_json::to_value(config).unwrap())
            .unwrap()
            .migrated();

        assert_eq!(restored.locked_sound_style, AlertSoundStyle::Crisp);
        assert_eq!(restored.unlocked_sound_style, AlertSoundStyle::Prominent);
    }

    #[test]
    fn defaults_use_resource_presets_and_full_alert_volume() {
        let config = AppConfig::default();

        assert_eq!(
            config.message_template,
            include_str!("../resources/presets/zh/combat1.txt")
        );
        assert_eq!(
            config.message_template_presets,
            [
                include_str!("../resources/presets/zh/combat1.txt"),
                include_str!("../resources/presets/zh/combat2.txt"),
                include_str!("../resources/presets/zh/combat1.txt"),
            ]
        );
        assert_eq!(
            config.round_report_template_presets,
            [
                include_str!("../resources/presets/zh/report1.txt"),
                include_str!("../resources/presets/zh/report2.txt"),
                include_str!("../resources/presets/zh/report1.txt"),
            ]
        );
        assert_eq!(config.alert_volume, 1.0);
    }

    #[test]
    fn resetting_selected_message_preset_preserves_everything_else() {
        let mut config = AppConfig::defaults_for_language(Language::English);
        config.select_message_template_preset(1);
        config.message_template = "unsaved message draft".to_owned();
        config.message_template_preset_names[1] = "My message".to_owned();
        config.message_template_presets[0] = "other message".to_owned();
        config.round_report_template = "report draft".to_owned();
        config.round_report_template_presets[0] = "other report".to_owned();
        let names_before = config.message_template_preset_names.clone();
        let reports_before = config.round_report_template_presets.clone();
        let report_draft_before = config.round_report_template.clone();

        assert!(config.reset_active_message_template_to_default());
        assert_eq!(
            config.message_template,
            include_str!("../resources/presets/en/combat2.txt")
        );
        assert_eq!(config.message_template_presets[1], config.message_template);
        assert_eq!(config.message_template_presets[0], "other message");
        assert_eq!(config.message_template_preset_names, names_before);
        assert_eq!(config.round_report_template_presets, reports_before);
        assert_eq!(config.round_report_template, report_draft_before);
        assert_eq!(config.active_message_template_preset, 1);
    }

    #[test]
    fn resetting_selected_report_preset_uses_current_language() {
        let mut config = AppConfig::defaults_for_language(Language::Chinese);
        config.select_round_report_template_preset(2);
        config.round_report_template = "未保存的战报草稿".to_owned();
        config.round_report_template_preset_names[2] = "自定义名称".to_owned();
        config.round_report_template_presets[0] = "另一份战报".to_owned();
        config.message_template = "局内消息草稿".to_owned();
        let names_before = config.round_report_template_preset_names.clone();
        let messages_before = config.message_template_presets.clone();
        let message_draft_before = config.message_template.clone();

        assert!(config.reset_active_round_report_template_to_default());
        assert_eq!(
            config.round_report_template,
            include_str!("../resources/presets/zh/report1.txt")
        );
        assert_eq!(
            config.round_report_template_presets[2],
            config.round_report_template
        );
        assert_eq!(config.round_report_template_presets[0], "另一份战报");
        assert_eq!(config.round_report_template_preset_names, names_before);
        assert_eq!(config.message_template_presets, messages_before);
        assert_eq!(config.message_template, message_draft_before);
        assert_eq!(config.active_round_report_template_preset, 2);
    }

    #[test]
    fn localized_defaults_use_localized_preset_names() {
        let chinese = AppConfig::defaults_for_language(Language::Chinese);
        let english = AppConfig::defaults_for_language(Language::English);

        assert_eq!(
            chinese.message_template_preset_names,
            ["预设 1", "预设 2", "预设 3"]
        );
        assert_eq!(
            chinese.round_report_template_preset_names,
            ["预设 1", "预设 2", "预设 3"]
        );
        assert_eq!(
            english.message_template_preset_names,
            ["Preset 1", "Preset 2", "Preset 3"]
        );
        assert_eq!(
            english.round_report_template_preset_names,
            ["Preset 1", "Preset 2", "Preset 3"]
        );
    }

    #[test]
    fn language_change_translates_only_untouched_builtin_templates_and_names() {
        let mut config = AppConfig::defaults_for_language(Language::Chinese);
        config.message_template_presets[2].push(' ');
        config.message_template_preset_names[1] = "我的预设".to_owned();
        config.round_report_template_presets[1] = "CUSTOM REPORT".to_owned();

        let localized = config.with_localized_defaults(Language::English);

        assert_eq!(localized.language, Language::English);
        assert_eq!(localized.message_template, DEFAULT_TEMPLATE_ENGLISH);
        assert_eq!(
            localized.message_template_presets[0],
            DEFAULT_TEMPLATE_ENGLISH
        );
        assert_eq!(
            localized.message_template_presets[1],
            DEFAULT_TEMPLATE_PRESET_2_ENGLISH
        );
        assert_eq!(
            localized.message_template_presets[2],
            format!("{DEFAULT_TEMPLATE} ")
        );
        assert_eq!(
            localized.message_template_preset_names,
            ["Preset 1", "我的预设", "Preset 3"]
        );
        assert_eq!(localized.round_report_template_presets[1], "CUSTOM REPORT");
    }

    #[test]
    fn localized_preset_resources_are_kept_in_matching_language_pairs() {
        fn filenames(language: &str) -> Vec<String> {
            let mut names = fs::read_dir(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("resources/presets")
                    .join(language),
            )
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
            names.sort();
            names
        }

        let expected = ["combat1.txt", "combat2.txt", "report1.txt", "report2.txt"];
        assert_eq!(filenames("zh"), expected);
        assert_eq!(filenames("en"), expected);
    }

    #[test]
    fn version_twelve_config_receives_original_alert_sounds() {
        let mut value = serde_json::to_value(AppConfig::default()).unwrap();
        let object = value.as_object_mut().unwrap();
        object.insert("version".to_owned(), serde_json::json!(12));
        object.remove("locked_sound_style");
        object.remove("unlocked_sound_style");

        let migrated = serde_json::from_value::<AppConfig>(value)
            .unwrap()
            .migrated();

        assert_eq!(migrated.version, CONFIG_VERSION);
        assert_eq!(migrated.locked_sound_style, AlertSoundStyle::Soft);
        assert_eq!(migrated.unlocked_sound_style, AlertSoundStyle::Soft);
    }

    #[test]
    fn overlay_dragging_is_one_consistent_semantic_state() {
        let mut config = AppConfig::default();
        assert!(!config.overlay_draggable());

        config.set_overlay_draggable(true);
        assert!(config.overlay_draggable());
        assert!(!config.overlay_locked);
        assert!(!config.overlay_mouse_passthrough);

        config.set_overlay_draggable(false);
        assert!(!config.overlay_draggable());
        assert!(config.overlay_locked);
        assert!(config.overlay_mouse_passthrough);

        config.overlay_locked = false;
        config.overlay_mouse_passthrough = true;
        assert!(!config.overlay_draggable());
    }

    #[test]
    fn overlay_scale_defaults_and_survives_serialization() {
        assert_eq!(AppConfig::default().overlay_scale, 1.0);

        let config = AppConfig {
            overlay_scale: 1.5,
            ..AppConfig::default()
        };
        let restored = serde_json::from_value::<AppConfig>(serde_json::to_value(config).unwrap())
            .unwrap()
            .migrated();

        assert_eq!(restored.overlay_scale, 1.5);
        restored.validate().unwrap();
    }

    #[test]
    fn old_config_receives_default_overlay_scale() {
        let mut value = serde_json::to_value(AppConfig::default()).unwrap();
        let object = value.as_object_mut().unwrap();
        object.insert("version".to_owned(), serde_json::json!(14));
        object.remove("overlay_scale");

        let migrated = serde_json::from_value::<AppConfig>(value)
            .unwrap()
            .migrated();

        assert_eq!(migrated.version, CONFIG_VERSION);
        assert_eq!(migrated.overlay_scale, 1.0);
    }

    #[test]
    fn rejects_unknown_template_variable() {
        let error = validate_template("{{unknown}}", Language::Chinese)
            .unwrap_err()
            .to_string();
        assert!(error.contains("未知变量"));
    }

    #[test]
    fn version_one_config_receives_default_round_report_template() {
        let mut value = serde_json::to_value(AppConfig::default()).unwrap();
        let object = value.as_object_mut().unwrap();
        object.insert("version".to_owned(), serde_json::json!(1));
        object.remove("round_report_template");

        let migrated = serde_json::from_value::<AppConfig>(value)
            .unwrap()
            .migrated();
        assert_eq!(migrated.version, CONFIG_VERSION);
        assert_eq!(
            migrated.round_report_template,
            DEFAULT_ROUND_REPORT_TEMPLATE
        );
    }

    #[test]
    fn version_five_builtin_templates_upgrade_to_new_round_metrics() {
        let migrated = AppConfig {
            version: 5,
            message_template: VERSION_5_DEFAULT_TEMPLATE.to_owned(),
            round_report_template: VERSION_5_DEFAULT_ROUND_REPORT_TEMPLATE.to_owned(),
            ..AppConfig::default()
        }
        .migrated();

        assert_eq!(migrated.message_template, DEFAULT_TEMPLATE);
        assert_eq!(
            migrated.round_report_template,
            DEFAULT_ROUND_REPORT_TEMPLATE
        );
    }

    #[test]
    fn migration_preserves_custom_templates() {
        let migrated = AppConfig {
            version: 5,
            message_template: "CUSTOM {{latest_dps}}".to_owned(),
            round_report_template: "CUSTOM REPORT {{round_total_damage}}".to_owned(),
            ..AppConfig::default()
        }
        .migrated();

        assert_eq!(migrated.message_template, "CUSTOM {{latest_dps}}");
        assert_eq!(
            migrated.round_report_template,
            "CUSTOM REPORT {{round_total_damage}}"
        );
    }

    #[test]
    fn version_six_templates_migrate_average_variable_typo() {
        let migrated = AppConfig {
            version: 6,
            message_template: "{{ave_dps}} {{round_ave_dps}} {{#if has_ave_dps}}{{#if has_round_ave_dps}}ok{{/if}}{{/if}}".to_owned(),
            round_report_template: "{{round_report_ave_dps}} {{#if has_round_report_ave_dps}}ok{{/if}}".to_owned(),
            ..AppConfig::default()
        }
        .migrated();

        assert_eq!(
            migrated.message_template,
            "{{avg_dps}} {{round_avg_dps}} {{#if has_avg_dps}}{{#if has_round_avg_dps}}ok{{/if}}{{/if}}"
        );
        assert_eq!(
            migrated.round_report_template,
            "{{round_report_avg_dps}} {{#if has_round_report_avg_dps}}ok{{/if}}"
        );
    }

    #[test]
    fn version_seven_templates_drop_redundant_presence_flags() {
        let migrated = AppConfig {
            version: 7,
            message_template: "{{#if has_boss_lock}}{{boss_lock}}{{/if}}|{{#if has_boss}}{{boss}}{{/if}}|{{#if has_no_wasd_for_10s}}{{#if no_wasd_for_10s}}idle{{else}}active{{/if}}{{/if}}".to_owned(),
            round_report_template: "{{#if has_round_total_damage}}{{round_total_damage}}{{/if}}|{{#if has_round_report_damage_taken}}{{round_report_damage_taken}}{{/if}}".to_owned(),
            ..AppConfig::default()
        }
        .migrated();

        assert_eq!(
            migrated.message_template,
            "{{#if boss_lock}}{{boss_lock}}{{/if}}|{{#if boss}}{{boss}}{{/if}}|{{#if true}}{{#if no_wasd_for_10s}}idle{{else}}active{{/if}}{{/if}}"
        );
        assert_eq!(
            migrated.round_report_template,
            "{{#if true}}{{round_total_damage}}{{/if}}|{{#if true}}{{round_report_damage_taken}}{{/if}}"
        );
        validate_template(&migrated.message_template, migrated.language).unwrap();
        validate_template(&migrated.round_report_template, migrated.language).unwrap();
    }

    #[test]
    fn version_eighteen_templates_remove_redundant_report_and_combat_duration_variables() {
        let old_template = "{{#if has_round_report}}report{{/if}}|{{round_combat_duration}}|{{#if has_round_combat_duration}}timed{{else}}no-time{{/if}}";
        let migrated = AppConfig {
            version: 18,
            message_template: "{{#if has_round_report}}wrong-context{{/if}}".to_owned(),
            message_template_presets: std::array::from_fn(|_| {
                "{{#if has_round_report}}wrong-context{{/if}}".to_owned()
            }),
            round_report_template: old_template.to_owned(),
            round_report_template_presets: std::array::from_fn(|_| old_template.to_owned()),
            active_round_report_template_preset: 0,
            ..AppConfig::default()
        }
        .migrated();

        assert_eq!(
            migrated.message_template,
            "{{#if false}}wrong-context{{/if}}"
        );
        assert_eq!(
            migrated.round_report_template,
            "{{#if true}}report{{/if}}||{{#if false}}timed{{else}}no-time{{/if}}"
        );
        assert!(
            migrated
                .round_report_template_presets
                .iter()
                .all(|template| template
                    == "{{#if true}}report{{/if}}||{{#if false}}timed{{else}}no-time{{/if}}")
        );
        validate_template(&migrated.round_report_template, migrated.language).unwrap();
    }

    #[test]
    fn version_nineteen_templates_remove_unreachable_context_variables() {
        let combat = "{{phase}}|{{status}}|{{waiting_for_next_round}}|{{#if (and (eq phase \"COMBAT\") (eq status \"LIVE\") (not waiting_for_next_round))}}ok{{/if}}";
        let report = "{{phase}}|{{status}}|{{#if (eq phase \"LOBBY\")}}report{{/if}}";
        let migrated = AppConfig {
            version: 19,
            message_template: combat.to_owned(),
            message_template_presets: std::array::from_fn(|_| combat.to_owned()),
            active_message_template_preset: 0,
            round_report_template: report.to_owned(),
            round_report_template_presets: std::array::from_fn(|_| report.to_owned()),
            active_round_report_template_preset: 0,
            ..AppConfig::default()
        }
        .migrated();

        assert_eq!(
            migrated.message_template,
            "COMBAT|LIVE||{{#if (and (eq \"COMBAT\" \"COMBAT\") (eq \"LIVE\" \"LIVE\") (not false))}}ok{{/if}}"
        );
        assert_eq!(
            migrated.round_report_template,
            "LOBBY|LIVE|{{#if (eq \"LOBBY\" \"LOBBY\")}}report{{/if}}"
        );
        validate_template(&migrated.message_template, migrated.language).unwrap();
        validate_template(&migrated.round_report_template, migrated.language).unwrap();
    }

    #[test]
    fn switching_presets_keeps_unsaved_edits_in_each_slot() {
        let mut config = AppConfig::default();
        config.message_template = "PRESET ONE".to_owned();

        assert!(config.select_message_template_preset(1));
        assert_eq!(config.message_template, DEFAULT_TEMPLATE_PRESET_2);
        config.message_template = "PRESET TWO".to_owned();

        assert!(config.select_message_template_preset(0));
        assert_eq!(config.message_template, "PRESET ONE");
        assert_eq!(config.message_template_presets[1], "PRESET TWO");
        assert!(!config.select_message_template_preset(MESSAGE_TEMPLATE_PRESET_COUNT));
    }

    #[test]
    fn version_eight_config_migrates_existing_template_into_first_preset() {
        let migrated = AppConfig {
            version: 8,
            message_template: "CUSTOM {{latest_dps}}".to_owned(),
            ..AppConfig::default()
        }
        .migrated();

        assert_eq!(migrated.active_message_template_preset, 0);
        assert_eq!(
            migrated.message_template_presets[0],
            "CUSTOM {{latest_dps}}"
        );
        assert_eq!(
            migrated.message_template_presets[1],
            DEFAULT_TEMPLATE_PRESET_2
        );
        assert_eq!(migrated.message_template_presets[2], DEFAULT_TEMPLATE);
    }

    #[test]
    fn selected_preset_and_all_template_texts_survive_serialization() {
        let mut config = AppConfig::default();
        config.message_template_preset_names =
            ["日常".to_owned(), "爆发".to_owned(), "辅助".to_owned()];
        config.message_template = "ONE".to_owned();
        config.select_message_template_preset(1);
        config.message_template = "TWO".to_owned();
        config.select_message_template_preset(2);
        config.message_template = "THREE".to_owned();
        config.sync_active_message_template_preset();

        let restored = serde_json::from_value::<AppConfig>(serde_json::to_value(config).unwrap())
            .unwrap()
            .migrated();

        assert_eq!(restored.active_message_template_preset, 2);
        assert_eq!(restored.message_template, "THREE");
        assert_eq!(restored.message_template_presets, ["ONE", "TWO", "THREE"]);
        assert_eq!(
            restored.message_template_preset_names,
            ["日常", "爆发", "辅助"]
        );
    }

    #[test]
    fn validation_checks_inactive_presets() {
        let mut config = AppConfig::default();
        config.message_template_presets[2] = "{{unknown}}".to_owned();

        let error = config.validate().unwrap_err().to_string();
        assert!(error.contains("预设 3"));
    }

    #[test]
    fn switching_round_report_presets_keeps_unsaved_edits_in_each_slot() {
        let mut config = AppConfig::default();
        config.round_report_template = "REPORT ONE".to_owned();

        assert!(config.select_round_report_template_preset(1));
        assert_eq!(
            config.round_report_template,
            DEFAULT_ROUND_REPORT_TEMPLATE_PRESET_2
        );
        config.round_report_template = "REPORT TWO".to_owned();

        assert!(config.select_round_report_template_preset(0));
        assert_eq!(config.round_report_template, "REPORT ONE");
        assert_eq!(config.round_report_template_presets[1], "REPORT TWO");
        assert!(!config.select_round_report_template_preset(ROUND_REPORT_TEMPLATE_PRESET_COUNT));
    }

    #[test]
    fn version_nine_config_migrates_existing_report_into_first_preset() {
        let migrated = AppConfig {
            version: 9,
            round_report_template: "REPORT {{round_total_damage}}".to_owned(),
            ..AppConfig::default()
        }
        .migrated();

        assert_eq!(migrated.active_round_report_template_preset, 0);
        assert_eq!(
            migrated.round_report_template_presets[0],
            "REPORT {{round_total_damage}}"
        );
        assert_eq!(
            migrated.round_report_template_presets[1],
            DEFAULT_ROUND_REPORT_TEMPLATE_PRESET_2
        );
        assert_eq!(
            migrated.round_report_template_presets[2],
            DEFAULT_ROUND_REPORT_TEMPLATE
        );
    }

    #[test]
    fn selected_report_preset_and_texts_survive_serialization() {
        let mut config = AppConfig::default();
        config.round_report_template_preset_names = [
            "简洁战报".to_owned(),
            "详细战报".to_owned(),
            "团队战报".to_owned(),
        ];
        config.round_report_template = "REPORT ONE".to_owned();
        config.select_round_report_template_preset(1);
        config.round_report_template = "REPORT TWO".to_owned();
        config.select_round_report_template_preset(2);
        config.round_report_template = "REPORT THREE".to_owned();
        config.sync_active_round_report_template_preset();

        let restored = serde_json::from_value::<AppConfig>(serde_json::to_value(config).unwrap())
            .unwrap()
            .migrated();

        assert_eq!(restored.active_round_report_template_preset, 2);
        assert_eq!(restored.round_report_template, "REPORT THREE");
        assert_eq!(
            restored.round_report_template_presets,
            ["REPORT ONE", "REPORT TWO", "REPORT THREE"]
        );
        assert_eq!(
            restored.round_report_template_preset_names,
            ["简洁战报", "详细战报", "团队战报"]
        );
    }

    #[test]
    fn version_eleven_config_receives_default_preset_names() {
        let mut value = serde_json::to_value(AppConfig::default()).unwrap();
        let object = value.as_object_mut().unwrap();
        object.insert("version".to_owned(), serde_json::json!(11));
        object.remove("message_template_preset_names");
        object.remove("round_report_template_preset_names");

        let migrated = serde_json::from_value::<AppConfig>(value)
            .unwrap()
            .migrated();

        assert_eq!(migrated.version, CONFIG_VERSION);
        assert_eq!(
            migrated.message_template_preset_names,
            ["预设 1", "预设 2", "预设 3"]
        );
        assert_eq!(
            migrated.round_report_template_preset_names,
            ["预设 1", "预设 2", "预设 3"]
        );
    }

    #[test]
    fn preset_names_must_be_non_empty_and_reasonably_short() {
        let mut config = AppConfig::default();
        config.message_template_preset_names[1] = "   ".to_owned();
        assert!(
            config
                .validate()
                .unwrap_err()
                .to_string()
                .contains("名称不能为空")
        );

        config.message_template_preset_names[1] = "x".repeat(TEMPLATE_PRESET_NAME_MAX_CHARS + 1);
        assert!(
            config
                .validate()
                .unwrap_err()
                .to_string()
                .contains("名称不能超过")
        );
    }

    #[test]
    fn validation_checks_inactive_report_presets() {
        let mut config = AppConfig::default();
        config.round_report_template_presets[1] = "{{unknown}}".to_owned();

        let error = config.validate().unwrap_err().to_string();
        assert!(error.contains("战报模板预设 2"));
    }

    #[test]
    fn version_ten_builtin_report_presets_gain_standstill_metric() {
        let old = VERSION_10_DEFAULT_ROUND_REPORT_TEMPLATE.to_owned();
        let migrated = AppConfig {
            version: 10,
            round_report_template: old.clone(),
            round_report_template_presets: std::array::from_fn(|_| old.clone()),
            ..AppConfig::default()
        }
        .migrated();

        assert_eq!(
            migrated.round_report_template,
            DEFAULT_ROUND_REPORT_TEMPLATE
        );
        assert!(
            migrated
                .round_report_template_presets
                .iter()
                .zip([
                    DEFAULT_ROUND_REPORT_TEMPLATE,
                    DEFAULT_ROUND_REPORT_TEMPLATE_PRESET_2,
                    DEFAULT_ROUND_REPORT_TEMPLATE,
                ])
                .all(|(template, expected)| template == expected)
        );
    }

    #[test]
    fn version_thirteen_builtin_presets_upgrade_without_replacing_custom_text() {
        let migrated = AppConfig {
            version: 13,
            message_template: VERSION_13_DEFAULT_TEMPLATE.to_owned(),
            message_template_presets: [
                VERSION_13_DEFAULT_TEMPLATE.to_owned(),
                "CUSTOM MESSAGE".to_owned(),
                VERSION_13_DEFAULT_TEMPLATE.to_owned(),
            ],
            round_report_template: VERSION_13_DEFAULT_ROUND_REPORT_TEMPLATE.to_owned(),
            round_report_template_presets: [
                VERSION_13_DEFAULT_ROUND_REPORT_TEMPLATE.to_owned(),
                "CUSTOM REPORT".to_owned(),
                VERSION_13_DEFAULT_ROUND_REPORT_TEMPLATE.to_owned(),
            ],
            alert_volume: VERSION_13_DEFAULT_ALERT_VOLUME,
            ..AppConfig::default()
        }
        .migrated();

        assert_eq!(
            migrated.message_template_presets,
            [DEFAULT_TEMPLATE, "CUSTOM MESSAGE", DEFAULT_TEMPLATE]
        );
        assert_eq!(
            migrated.round_report_template_presets,
            [
                DEFAULT_ROUND_REPORT_TEMPLATE,
                "CUSTOM REPORT",
                DEFAULT_ROUND_REPORT_TEMPLATE,
            ]
        );
        assert_eq!(migrated.alert_volume, 1.0);
    }

    #[test]
    fn version_sixteen_english_meme_presets_upgrade_without_replacing_custom_text() {
        let migrated = AppConfig {
            version: 16,
            language: Language::English,
            message_template: VERSION_16_DEFAULT_TEMPLATE_ENGLISH.to_owned(),
            message_template_presets: [
                VERSION_16_DEFAULT_TEMPLATE_ENGLISH.to_owned(),
                VERSION_16_DEFAULT_TEMPLATE_PRESET_2_ENGLISH.to_owned(),
                "CUSTOM MEME".to_owned(),
            ],
            ..AppConfig::default()
        }
        .migrated();

        assert_eq!(migrated.message_template, DEFAULT_TEMPLATE_ENGLISH);
        assert_eq!(
            migrated.message_template_presets,
            [
                DEFAULT_TEMPLATE_ENGLISH,
                DEFAULT_TEMPLATE_PRESET_2_ENGLISH,
                "CUSTOM MEME"
            ]
        );
    }

    #[test]
    fn version_sixteen_modified_english_templates_are_never_replaced() {
        let customized_first = format!("{VERSION_16_DEFAULT_TEMPLATE_ENGLISH} ");
        let customized_second = VERSION_16_DEFAULT_TEMPLATE_PRESET_2_ENGLISH.replacen(
            "No damage",
            "Still no damage",
            1,
        );
        let migrated = AppConfig {
            version: 16,
            language: Language::English,
            message_template: customized_first.clone(),
            message_template_presets: [
                customized_first.clone(),
                customized_second.clone(),
                "CUSTOM MEME".to_owned(),
            ],
            ..AppConfig::default()
        }
        .migrated();

        assert_eq!(migrated.message_template, customized_first);
        assert_eq!(migrated.message_template_presets[0], customized_first);
        assert_eq!(migrated.message_template_presets[1], customized_second);
        assert_eq!(migrated.message_template_presets[2], "CUSTOM MEME");
    }

    #[test]
    fn version_seventeen_chinese_report_unit_is_localized_without_replacing_custom_text() {
        let mut old = AppConfig::defaults_for_language(Language::Chinese);
        old.version = 17;
        old.round_report_template = VERSION_17_DEFAULT_ROUND_REPORT_TEMPLATE_PRESET_2.to_owned();
        old.round_report_template_presets[1] =
            VERSION_17_DEFAULT_ROUND_REPORT_TEMPLATE_PRESET_2.to_owned();
        old.active_round_report_template_preset = 1;

        let migrated = old.migrated();
        assert_eq!(migrated.version, CONFIG_VERSION);
        assert_eq!(
            migrated.round_report_template,
            DEFAULT_ROUND_REPORT_TEMPLATE_PRESET_2
        );
        assert_eq!(
            migrated.round_report_template_presets[1],
            DEFAULT_ROUND_REPORT_TEMPLATE_PRESET_2
        );

        let mut custom = AppConfig::defaults_for_language(Language::Chinese);
        custom.version = 17;
        custom.round_report_template_presets[1] =
            format!("{VERSION_17_DEFAULT_ROUND_REPORT_TEMPLATE_PRESET_2} ");
        custom.active_round_report_template_preset = 1;
        custom.round_report_template = custom.round_report_template_presets[1].clone();
        assert_eq!(
            custom.migrated().round_report_template,
            format!("{VERSION_17_DEFAULT_ROUND_REPORT_TEMPLATE_PRESET_2} ")
        );
    }

    #[test]
    fn version_fifteen_builtin_reports_gain_step_estimate_without_replacing_custom_text() {
        let migrated = AppConfig {
            version: 15,
            round_report_template: VERSION_15_DEFAULT_ROUND_REPORT_TEMPLATE.to_owned(),
            round_report_template_presets: [
                VERSION_15_DEFAULT_ROUND_REPORT_TEMPLATE.to_owned(),
                VERSION_15_DEFAULT_ROUND_REPORT_TEMPLATE_PRESET_2.to_owned(),
                "CUSTOM {{current_step}}".to_owned(),
            ],
            active_round_report_template_preset: 0,
            ..AppConfig::default()
        }
        .migrated();

        assert_eq!(migrated.version, CONFIG_VERSION);
        assert_eq!(
            migrated.round_report_template_presets,
            [
                DEFAULT_ROUND_REPORT_TEMPLATE,
                DEFAULT_ROUND_REPORT_TEMPLATE_PRESET_2,
                "CUSTOM {{current_step}}",
            ]
        );
        assert_eq!(
            migrated.round_report_template,
            DEFAULT_ROUND_REPORT_TEMPLATE
        );
    }
}
