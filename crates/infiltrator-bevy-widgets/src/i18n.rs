//! Internationalization, locale formatting, and RTL mirroring support.

use bevy::ecs::resource::Resource;
use infiltrator_shared::locales::{Lang, Localizer};
use std::borrow::Cow;

/// Standard supported locales.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Locale {
    #[default]
    ZhCn,
    ZhTw,
    EnUs,
    JaJp,
    RuRu,
}

impl Locale {
    pub fn is_rtl(&self) -> bool {
        false // Extensible for Arabic/Hebrew
    }

    pub fn code(&self) -> &'static str {
        match self {
            Locale::ZhCn => "zh-CN",
            Locale::ZhTw => "zh-TW",
            Locale::EnUs => "en-US",
            Locale::JaJp => "ja-JP",
            Locale::RuRu => "ru-RU",
        }
    }
}

/// Typed locale translation keys eliminating hardcoded strings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LocaleKey {
    Overview,
    Proxies,
    Profiles,
    Rules,
    Dns,
    Connections,
    Logs,
    Doctor,
    Settings,
    Sync,
    AppRouting,
    ModeRule,
    ModeGlobal,
    ModeDirect,
    StatusRunning,
    StatusStopped,
    StatusUnavailable,
    ActionSave,
    ActionCancel,
    ActionDelete,
    ActionConfirm,
    ActionUpdate,
}

impl LocaleKey {
    pub const fn key(self) -> &'static str {
        match self {
            Self::Overview => "nav_overview",
            Self::Proxies => "nav_proxies",
            Self::Profiles => "nav_profiles",
            Self::Rules => "nav_rules",
            Self::Dns => "nav_dns",
            Self::Connections => "nav_connections",
            Self::Logs => "nav_logs",
            Self::Doctor => "nav_doctor",
            Self::Settings => "nav_settings",
            Self::Sync => "nav_sync",
            Self::AppRouting => "nav_app_routing",
            Self::ModeRule => "mode_rule",
            Self::ModeGlobal => "mode_global",
            Self::ModeDirect => "mode_direct",
            Self::StatusRunning => "status_running",
            Self::StatusStopped => "status_stopped",
            Self::StatusUnavailable => "common_unavailable",
            Self::ActionSave => "btn_save",
            Self::ActionCancel => "btn_cancel",
            Self::ActionDelete => "delete",
            Self::ActionConfirm => "common_confirm",
            Self::ActionUpdate => "btn_update",
        }
    }
}

/// Locale adapter over the sole shared resource catalogue; no widget-owned copy table.
#[derive(Resource, Clone, Debug, Default)]
pub struct TranslationRepo {
    pub current_locale: Locale,
}

impl TranslationRepo {
    pub fn new(locale: Locale) -> Self {
        Self {
            current_locale: locale,
        }
    }

    pub fn translate(&self, key: LocaleKey) -> Cow<'static, str> {
        Lang(self.current_locale.code()).tr(key.key())
    }
}

/// Format bytes into human-readable representation.
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if bytes >= TB {
        format!("{:.2} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Format transfer rate in bytes per second.
pub fn format_rate(bytes_per_sec: f64) -> String {
    format!("{}/s", format_bytes(bytes_per_sec.max(0.0) as u64))
}

/// Format duration in seconds into human-readable timestamp.
pub fn format_duration_secs(secs: u64) -> String {
    let hours = secs / 3600;
    let mins = (secs % 3600) / 60;
    let rem_secs = secs % 60;

    if hours > 0 {
        format!("{:02}:{:02}:{:02}", hours, mins, rem_secs)
    } else {
        format!("{:02}:{:02}", mins, rem_secs)
    }
}
