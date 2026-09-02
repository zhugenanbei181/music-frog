use std::collections::HashMap;

pub struct I18nInterpolator;

impl I18nInterpolator {
    pub fn interpolate(template: &str, params: &HashMap<String, String>) -> String {
        let mut result = String::new();
        let mut chars = template.chars().peekable();

        while let Some(c) = chars.next() {
            if c == '{' {
                let mut key = String::new();
                let mut found_closing = false;

                while let Some(&next_c) = chars.peek() {
                    chars.next();
                    if next_c == '}' {
                        found_closing = true;
                        break;
                    }
                    key.push(next_c);
                }

                if found_closing {
                    if let Some(val) = params.get(&key) {
                        result.push_str(val);
                    } else {
                        result.push('{');
                        result.push_str(&key);
                        result.push('}');
                    }
                } else {
                    result.push('{');
                    result.push_str(&key);
                }
            } else {
                result.push(c);
            }
        }
        result
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
