use std::borrow::Cow;

#[path = "locales_table.rs"]
mod locales_table;
#[path = "locales_table_en.rs"]
mod locales_table_en;
#[path = "locales_table_en_ext.rs"]
mod locales_table_en_ext;
#[path = "locales_table_ext.rs"]
mod locales_table_ext;

#[path = "locales_table_workflow_en.rs"]
mod workflow_en;
#[path = "locales_table_workflow_zh.rs"]
mod workflow_zh;

use locales_table::translate_zh_cn;
use locales_table_en::translate_en;
use locales_table_en_ext::translate_en_ext;
use locales_table_ext::translate_zh_cn_ext;
#[path = "locales_table_network_en.rs"]
mod network_en;
#[path = "locales_table_network_zh.rs"]
mod network_zh;
use network_en::translate_network_en;
use network_zh::translate_network_zh;

use workflow_en::translate_workflow_en;
use workflow_zh::translate_workflow_zh;

#[path = "locales_table_surface_en.rs"]
mod surface_en;
#[path = "locales_table_surface_zh.rs"]
mod surface_zh;
use surface_en::translate_surface_en;
use surface_zh::translate_surface_zh;

#[path = "locales_table_errors_en.rs"]
mod errors_en;
#[path = "locales_table_errors_zh.rs"]
mod errors_zh;
use errors_en::translate_errors_en;
use errors_zh::translate_errors_zh;

pub trait Localizer {
    fn tr(&self, key: &str) -> Cow<'static, str>;
}

pub struct Lang<'a>(pub &'a str);

impl Localizer for Lang<'_> {
    fn tr(&self, key: &str) -> Cow<'static, str> {
        let tables = match self.0 {
            "en-US" | "en" => [
                translate_en,
                translate_en_ext,
                translate_workflow_en,
                translate_network_en,
                translate_surface_en,
                translate_errors_en,
            ],
            _ => [
                translate_zh_cn,
                translate_zh_cn_ext,
                translate_workflow_zh,
                translate_network_zh,
                translate_surface_zh,
                translate_errors_zh,
            ],
        };
        for translate in tables {
            let copy = translate(key);
            if copy.as_ref() != key {
                return copy;
            }
        }
        key.to_owned().into()
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
        resolve_system_language().unwrap_or_else(get_system_language)
    } else if value.eq_ignore_ascii_case("en")
        || value.eq_ignore_ascii_case("en-us")
        || value.eq_ignore_ascii_case("en_us")
    {
        "en-US".to_string()
    } else if value.eq_ignore_ascii_case("zh")
        || value.eq_ignore_ascii_case("zh-cn")
        || value.eq_ignore_ascii_case("zh_cn")
    {
        "zh-CN".into()
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
