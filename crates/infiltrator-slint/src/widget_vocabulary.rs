//! The vocabulary the imported Lumen widgets speak.
//!
//! The imported widgets name a sort dimension and a level with plain
//! strings. Those strings are presentation vocabulary, not business state:
//! the shared typed keys own the meaning, and this module is the one adapter
//! between the two. No other place in the surface may translate a widget
//! string on its own, so the widget vocabulary can never grow a second
//! meaning for one control.

use infiltrator_contract::logs::LogLevel;
use infiltrator_domain::connection_view::ConnectionSortKey;

/// The shared sort key the widget's sort vocabulary names.
///
/// The widget offers four dimensions and every one of them names one shared
/// key, so the connection rows are ordered by the shared reduction and not
/// by a second surface-local order. A value the widget does not offer names
/// the shared default key, because a control that names no dimension has no
/// order of its own.
pub fn connection_sort_key(value: &str) -> ConnectionSortKey {
    match value {
        "upload" => ConnectionSortKey::UploadDesc,
        "download" => ConnectionSortKey::DownloadDesc,
        "host" => ConnectionSortKey::HostAsc,
        "time" => ConnectionSortKey::LatestDesc,
        _ => ConnectionSortKey::default(),
    }
}

/// The widget's level vocabulary for the shared level filter.
///
/// `None` is "every level". The widget has no button for the `unknown`
/// record level, so an owner filter on it reads as `unknown` and no level
/// button is selected; the surface never claims a level the owner did not
/// confirm.
pub fn log_level_value(level: Option<LogLevel>) -> &'static str {
    match level {
        None => "all",
        Some(LogLevel::Debug) => "debug",
        Some(LogLevel::Info) => "info",
        Some(LogLevel::Warn) => "warning",
        Some(LogLevel::Error) => "error",
        Some(LogLevel::Unknown) => "unknown",
    }
}
