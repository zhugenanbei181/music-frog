use crate::locales::{Lang, Localizer};
use std::collections::HashMap;

/// Render canonical copy once, leaving inserted values opaque.
pub fn localize(code: &str, key: &str, values: &[(&str, String)]) -> String {
    let values: Vec<_> = values
        .iter()
        .map(|(key, value)| (*key, value.as_str()))
        .collect();
    interpolate(Lang(code).tr(key).as_ref(), &values)
}
/// Convenient array-based key-value interpolation helper.
pub fn interpolate(template: &str, pairs: &[(&str, &str)]) -> String {
    interpolate_with(template, |key| {
        pairs
            .iter()
            .find(|(name, _)| *name == key)
            .map(|(_, value)| *value)
    })
}

/// Resolve placeholders in the template once; inserted user values remain opaque.
fn interpolate_with<'a>(template: &str, mut lookup: impl FnMut(&str) -> Option<&'a str>) -> String {
    let mut output = String::with_capacity(template.len());
    let mut remaining = template;
    while let Some(start) = remaining.find('{') {
        output.push_str(&remaining[..start]);
        let tail = &remaining[start + 1..];
        let Some(end) = tail.find('}') else {
            output.push_str(&remaining[start..]);
            return output;
        };
        if let Some(value) = lookup(&tail[..end]) {
            output.push_str(value);
        } else {
            output.push_str(&remaining[start..start + end + 2]);
        }
        remaining = &tail[end + 1..];
    }
    output.push_str(remaining);
    output
}

pub struct I18nInterpolator;

impl I18nInterpolator {
    pub fn interpolate(template: &str, params: &HashMap<String, String>) -> String {
        interpolate_with(template, |key| params.get(key).map(String::as_str))
    }

    pub fn resolve_fallback_locale<'a>(preferred: &str, supported: &[&'a str]) -> &'a str {
        // Exact match
        for &loc in supported {
            if loc == preferred {
                return loc;
            }
        }

        // Language subtag match
        let preferred_lang = preferred.split('-').next().unwrap_or(preferred);
        for &loc in supported {
            let loc_lang = loc.split('-').next().unwrap_or(loc);
            if loc_lang == preferred_lang {
                return loc;
            }
        }

        // Default fallback
        for &loc in supported {
            if loc == "en-US" {
                return loc;
            }
        }

        "en-US"
    }

    pub fn pluralize(count: usize, zero: &str, one: &str, other: &str) -> String {
        match count {
            0 => zero.to_string(),
            1 => one.to_string(),
            _ => other.replace("{count}", &count.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_interpolation_interfaces_keep_user_placeholders_opaque_and_order_independent() {
        let template = "{user} · {count}";
        let pairs = [("user", "User {count} / 日本"), ("count", "3")];
        assert_eq!(interpolate(template, &pairs), "User {count} / 日本 · 3");
        assert_eq!(
            interpolate(template, &[pairs[1], pairs[0]]),
            "User {count} / 日本 · 3"
        );
        let params = pairs
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value.to_owned()))
            .collect();
        assert_eq!(
            I18nInterpolator::interpolate(template, &params),
            "User {count} / 日本 · 3"
        );
        assert_eq!(
            interpolate("{known} {missing} {unfinished", &[("known", "value")]),
            "value {missing} {unfinished"
        );
    }

    #[test]
    fn test_interpolate_single() {
        let mut params = HashMap::new();
        params.insert("name".to_string(), "Alice".to_string());
        assert_eq!(
            I18nInterpolator::interpolate("Hello {name}!", &params),
            "Hello Alice!"
        );
    }

    #[test]
    fn test_interpolate_multiple() {
        let mut params = HashMap::new();
        params.insert("name".to_string(), "Alice".to_string());
        params.insert("greeting".to_string(), "Hi".to_string());
        assert_eq!(
            I18nInterpolator::interpolate("{greeting} {name}!", &params),
            "Hi Alice!"
        );
    }

    #[test]
    fn test_interpolate_missing() {
        let mut params = HashMap::new();
        params.insert("name".to_string(), "Alice".to_string());
        assert_eq!(
            I18nInterpolator::interpolate("Hello {name}, where is {missing}?", &params),
            "Hello Alice, where is {missing}?"
        );
    }

    #[test]
    fn test_resolve_fallback_locale_exact() {
        let supported = vec!["en-US", "zh-CN", "zh-HK", "fr-FR"];
        assert_eq!(
            I18nInterpolator::resolve_fallback_locale("zh-HK", &supported),
            "zh-HK"
        );
    }

    #[test]
    fn test_resolve_fallback_locale_subtag() {
        let supported = vec!["en-US", "zh-CN", "fr-FR"];
        assert_eq!(
            I18nInterpolator::resolve_fallback_locale("zh-TW", &supported),
            "zh-CN"
        );
    }

    #[test]
    fn test_resolve_fallback_locale_default() {
        let supported = vec!["en-US", "zh-CN"];
        assert_eq!(
            I18nInterpolator::resolve_fallback_locale("es-ES", &supported),
            "en-US"
        );
    }

    #[test]
    fn test_pluralize_zero() {
        assert_eq!(
            I18nInterpolator::pluralize(0, "No apples", "One apple", "{count} apples"),
            "No apples"
        );
    }

    #[test]
    fn test_pluralize_one() {
        assert_eq!(
            I18nInterpolator::pluralize(1, "No apples", "One apple", "{count} apples"),
            "One apple"
        );
    }

    #[test]
    fn test_pluralize_other() {
        assert_eq!(
            I18nInterpolator::pluralize(5, "No apples", "One apple", "{count} apples"),
            "5 apples"
        );
    }
}
