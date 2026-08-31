use std::borrow::Cow;

#[path = "locales_table.rs"]
mod locales_table;
#[path = "locales_table_en.rs"]
mod locales_table_en;
use locales_table::translate_zh_cn;
use locales_table_en::translate_en;

pub trait Localizer {
    fn tr(&self, key: &str) -> Cow<'static, str>;
}

pub struct Lang<'a>(pub &'a str);

impl<'a> Localizer for Lang<'a> {
    fn tr(&self, key: &str) -> Cow<'static, str> {
        match self.0 {
            "en-US" | "en" => translate_en(key),
            _ => translate_zh_cn(key),
        }
    }
}

/// Best-effort system language detection, limited to the supported
/// `"zh-CN"` / `"en-US"` set.
pub fn get_system_language() -> String {
    if let Some(locale) = sys_locale::get_locale() {
        let normalized = locale.trim().to_ascii_lowercase();
        if normalized.starts_with("zh") {
            "zh-CN".to_string()
        } else {
            "en-US".to_string()
        }
    } else {
        "en-US".to_string()
    }
}

/// Resolves a stored language preference (`"system"`, `"en"`, or a raw
/// locale code) into a concrete language code.
pub fn resolve_language_code(value: &str) -> String {
    if value.eq_ignore_ascii_case("system") {
        resolve_system_language().unwrap_or_else(|| "zh-CN".to_string())
    } else if value.eq_ignore_ascii_case("en") {
        "en-US".to_string()
    } else {
        value.to_string()
    }
}

fn resolve_system_language() -> Option<String> {
    let locale = sys_locale::get_locale()?;
    Some(normalize_locale(&locale))
}

fn normalize_locale(locale: &str) -> String {
    let normalized = locale.trim().to_ascii_lowercase();
    if normalized.starts_with("zh") {
        "zh-CN".to_string()
    } else {
        "en-US".to_string()
    }
}
