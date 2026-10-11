use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::LazyLock;

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

static PRODUCT_ZH_CN_CATALOG: LazyLock<HashMap<String, String>> = LazyLock::new(|| {
    parse_po(include_str!(
        "../locales/product/translations/zh-CN/LC_MESSAGES/musicfrog.po"
    ))
});
static PRODUCT_EN_CATALOG: LazyLock<HashMap<String, String>> = LazyLock::new(|| {
    parse_po(include_str!(
        "../locales/product/translations/en/LC_MESSAGES/musicfrog.po"
    ))
});

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
        if let Some(copy) = translate_product_catalog(self.0, key) {
            return copy.into();
        }
        key.to_owned().into()
    }
}

/// Product gettext message IDs are shared across compiler-driven and
/// Rust-rendered surfaces; toolkit code does not own translation resources.
fn translate_product_catalog(locale: &str, message_id: &str) -> Option<&'static str> {
    let catalog = if locale.starts_with("en") {
        &PRODUCT_EN_CATALOG
    } else {
        &PRODUCT_ZH_CN_CATALOG
    };
    catalog.get(message_id).map(String::as_str)
}

fn parse_po(source: &str) -> HashMap<String, String> {
    #[derive(Default)]
    struct Entry {
        message_id: Option<String>,
        translation: Option<String>,
        active_field: Option<PoField>,
    }

    #[derive(Clone, Copy)]
    enum PoField {
        MessageId,
        Translation,
    }

    fn flush(entry: &mut Entry, translations: &mut HashMap<String, String>) {
        if let (Some(message_id), Some(translation)) =
            (entry.message_id.take(), entry.translation.take())
            && !message_id.is_empty()
            && !translation.is_empty()
        {
            translations.insert(message_id, translation);
        }
        entry.active_field = None;
    }

    fn decode_po_string(source: &str) -> Option<String> {
        serde_json::from_str(source).ok()
    }

    let mut translations = HashMap::new();
    let mut entry = Entry::default();
    for line in source.lines().map(str::trim) {
        if line.is_empty() {
            flush(&mut entry, &mut translations);
            continue;
        }
        if line.starts_with('#') || line.starts_with("msgctxt ") {
            continue;
        }
        if let Some(value) = line.strip_prefix("msgid ") {
            if entry.message_id.is_some() || entry.translation.is_some() {
                flush(&mut entry, &mut translations);
            }
            entry.message_id = decode_po_string(value);
            entry.active_field = Some(PoField::MessageId);
        } else if let Some(value) = line.strip_prefix("msgstr ") {
            entry.translation = decode_po_string(value);
            entry.active_field = Some(PoField::Translation);
        } else if line.starts_with("msgid_plural ") || line.starts_with("msgstr[") {
            entry.active_field = None;
        } else if line.starts_with('"') {
            let Some(value) = decode_po_string(line) else {
                continue;
            };
            match entry.active_field {
                Some(PoField::MessageId) => {
                    entry
                        .message_id
                        .get_or_insert_with(String::new)
                        .push_str(&value);
                }
                Some(PoField::Translation) => {
                    entry
                        .translation
                        .get_or_insert_with(String::new)
                        .push_str(&value);
                }
                None => {}
            }
        }
    }
    flush(&mut entry, &mut translations);
    translations
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

#[cfg(test)]
mod product_catalog_tests {
    use super::{Lang, Localizer, parse_po};

    #[test]
    fn gettext_parser_keeps_adjacent_entries_without_blank_separators() {
        let catalog =
            parse_po("msgid \"first\"\nmsgstr \"First\"\nmsgid \"second\"\nmsgstr \"Second\"\n");
        assert_eq!(catalog.get("first").map(String::as_str), Some("First"));
        assert_eq!(catalog.get("second").map(String::as_str), Some("Second"));
    }

    #[test]
    fn product_gettext_catalog_is_available_through_the_shared_localizer() {
        assert_eq!(Lang("en-US").tr("护眼森林"), "Forest");
        assert_eq!(Lang("zh-CN").tr("护眼森林"), "护眼森林");
        assert_eq!(
            Lang("en-US")
                .tr("界面改编自 Lumen；上游 README 声明采用 MIT，导入归档未附独立许可证文本。"),
            "The interface is adapted from Lumen. Its upstream README says MIT, but the imported archive did not include a separate license text."
        );
    }
}
