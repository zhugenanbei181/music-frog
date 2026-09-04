//! Internationalization, locale formatting, and RTL mirroring support.

use bevy::ecs::resource::Resource;
use std::collections::HashMap;

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
    pub fn fallback_str(&self) -> &'static str {
        match self {
            LocaleKey::Overview => "核心概览",
            LocaleKey::Proxies => "节点策略",
            LocaleKey::Profiles => "配置订阅",
            LocaleKey::Rules => "分流规则",
            LocaleKey::Dns => "DNS 拓扑",
            LocaleKey::Connections => "连接审计",
            LocaleKey::Logs => "运行日志",
            LocaleKey::Doctor => "智能体检",
            LocaleKey::Settings => "系统设置",
            LocaleKey::Sync => "数据同步",
            LocaleKey::AppRouting => "应用分流",
            LocaleKey::ModeRule => "规则模式",
            LocaleKey::ModeGlobal => "全局模式",
            LocaleKey::ModeDirect => "直连模式",
            LocaleKey::StatusRunning => "运行中",
            LocaleKey::StatusStopped => "已停止",
            LocaleKey::StatusUnavailable => "不可用",
            LocaleKey::ActionSave => "保存",
            LocaleKey::ActionCancel => "取消",
            LocaleKey::ActionDelete => "删除",
            LocaleKey::ActionConfirm => "确认",
            LocaleKey::ActionUpdate => "更新",
        }
    }
}

/// Global translation repository resource.
#[derive(Resource, Clone, Debug, Default)]
pub struct TranslationRepo {
    pub current_locale: Locale,
    pub strings: HashMap<(Locale, LocaleKey), String>,
}

impl TranslationRepo {
    pub fn new(locale: Locale) -> Self {
        Self {
            current_locale: locale,
            strings: HashMap::new(),
        }
    }

    pub fn translate(&self, key: LocaleKey) -> &str {
        if let Some(s) = self.strings.get(&(self.current_locale, key)) {
            s.as_str()
        } else {
            key.fallback_str()
        }
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
