use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum ClipboardContentType {
    SubscriptionUrl(String),
    Base64Profile(String),
    YamlConfig(String),
    JsonConfig(String),
    Unknown(String),
}

pub struct ClipboardHelper;

impl ClipboardHelper {
    /// Classifies the clipboard text into a recognized content type.
    pub fn classify_clipboard_text(text: &str) -> ClipboardContentType {
        let clean_text = Self::sanitize_clipboard_text(text);
        
        if clean_text.starts_with("clash://") || clean_text.starts_with("http://") || clean_text.starts_with("https://") {
            return ClipboardContentType::SubscriptionUrl(clean_text);
        }

        let is_json = clean_text.starts_with('{') && clean_text.ends_with('}');
        let has_json_keywords = clean_text.contains("\"proxies\":") || clean_text.contains("\"rules\":");
        if is_json && has_json_keywords {
            return ClipboardContentType::JsonConfig(clean_text);
        }

        let has_yaml_keywords = clean_text.contains("\nproxies:") || clean_text.starts_with("proxies:") 
                             || clean_text.contains("\nrules:") || clean_text.starts_with("rules:");
        if !is_json && has_yaml_keywords {
            return ClipboardContentType::YamlConfig(clean_text);
        }

        // Base64 check: typically base64 encoded strings for node lists
        let no_space: String = clean_text.chars().filter(|c| !c.is_whitespace()).collect();
        let is_base64_chars = no_space.chars().all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=');
        let has_padding_or_valid_len = no_space.len() % 4 == 0 && no_space.len() >= 4;
        
        if is_base64_chars && has_padding_or_valid_len && no_space.len() > 16 && !no_space.contains('{') && !no_space.contains(':') {
            return ClipboardContentType::Base64Profile(clean_text);
        }

        ClipboardContentType::Unknown(clean_text)
    }

    /// Sanitizes the clipboard text by stripping BOM, zero-width spaces, and outer whitespace.
    pub fn sanitize_clipboard_text(text: &str) -> String {
        text.trim_matches(|c: char| c.is_whitespace() || c == '\u{FEFF}' || c == '\u{200B}').to_string()
    }

    /// Extracts a subscription URL from a raw URL or clash scheme URI.
    pub fn extract_subscription_url(text: &str) -> Option<String> {
        let clean_text = Self::sanitize_clipboard_text(text);
        
        if clean_text.starts_with("clash://install-config?url=") {
            let url_part = clean_text.trim_start_matches("clash://install-config?url=");
            let end_idx = url_part.find('&').unwrap_or(url_part.len());
            let encoded_url = &url_part[..end_idx];
            return Some(Self::percent_decode(encoded_url));
        } else if clean_text.starts_with("http://") || clean_text.starts_with("https://") {
            return Some(clean_text);
        }

        None
    }

    /// Helper to percent-decode URL components.
    fn percent_decode(s: &str) -> String {
        let mut res = String::with_capacity(s.len());
        let mut bytes = s.as_bytes().iter().copied();
        let mut utf8_buffer = Vec::new();

        while let Some(b) = bytes.next() {
            if b == b'%' {
                if let (Some(h1), Some(h2)) = (bytes.next(), bytes.next()) {
                    let hex_bytes = [h1, h2];
                    if let Ok(hex_str) = std::str::from_utf8(&hex_bytes) {
                        if let Ok(byte) = u8::from_str_radix(hex_str, 16) {
                            utf8_buffer.push(byte);
                            continue;
                        }
                    }
                    // Invalid hex, push the accumulated buffer and the literal chars
                    if !utf8_buffer.is_empty() {
                        res.push_str(&String::from_utf8_lossy(&utf8_buffer));
                        utf8_buffer.clear();
                    }
                    res.push('%');
                    res.push(h1 as char);
                    res.push(h2 as char);
                } else {
                    res.push('%');
                    break; // End of string
                }
            } else {
                if !utf8_buffer.is_empty() {
                    res.push_str(&String::from_utf8_lossy(&utf8_buffer));
                    utf8_buffer.clear();
                }
                res.push(b as char);
            }
        }
        if !utf8_buffer.is_empty() {
            res.push_str(&String::from_utf8_lossy(&utf8_buffer));
        }
        res
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_clipboard_text() {
        assert_eq!(ClipboardHelper::sanitize_clipboard_text("  hello  "), "hello");
        assert_eq!(ClipboardHelper::sanitize_clipboard_text("\u{FEFF}hello\u{200B}"), "hello");
        assert_eq!(ClipboardHelper::sanitize_clipboard_text("\r\n  hello \t"), "hello");
    }

    #[test]
    fn test_classify_url() {
        assert_eq!(
            ClipboardHelper::classify_clipboard_text("https://example.com/sub"),
            ClipboardContentType::SubscriptionUrl("https://example.com/sub".into())
        );
        assert_eq!(
            ClipboardHelper::classify_clipboard_text("clash://install-config?url=https%3A%2F%2Fexample.com"),
            ClipboardContentType::SubscriptionUrl("clash://install-config?url=https%3A%2F%2Fexample.com".into())
        );
    }

    #[test]
    fn test_classify_json() {
        let json_text = r#"{"proxies": [{"name": "node1"}], "rules": []}"#;
        assert_eq!(
            ClipboardHelper::classify_clipboard_text(json_text),
            ClipboardContentType::JsonConfig(json_text.into())
        );
    }

    #[test]
    fn test_classify_yaml() {
        let yaml_text = "proxies:\n  - name: node1\nrules:\n  - MATCH,DIRECT";
        assert_eq!(
            ClipboardHelper::classify_clipboard_text(yaml_text),
            ClipboardContentType::YamlConfig(yaml_text.into())
        );
    }

    #[test]
    fn test_classify_base64() {
        let b64 = "dm1lc3M6Ly9leGFtcGxlCg=="; // len 24
        assert_eq!(
            ClipboardHelper::classify_clipboard_text(b64),
            ClipboardContentType::Base64Profile(b64.into())
        );
    }
    
    #[test]
    fn test_classify_unknown() {
        let text = "just some random text";
        assert_eq!(
            ClipboardHelper::classify_clipboard_text(text),
            ClipboardContentType::Unknown(text.into())
        );
    }

    #[test]
    fn test_extract_subscription_url() {
        assert_eq!(
            ClipboardHelper::extract_subscription_url("clash://install-config?url=https%3A%2F%2Fexample.com%2Fsub&name=test"),
            Some("https://example.com/sub".into())
        );
        assert_eq!(
            ClipboardHelper::extract_subscription_url("https://example.com/sub"),
            Some("https://example.com/sub".into())
        );
        assert_eq!(
            ClipboardHelper::extract_subscription_url("clash://install-config?url=https%3A%2F%2Fexample.com"),
            Some("https://example.com".into())
        );
        assert_eq!(
            ClipboardHelper::extract_subscription_url("invalid://url"),
            None
        );
    }

    #[test]
    fn test_percent_decode() {
        assert_eq!(ClipboardHelper::percent_decode("https%3A%2F%2Fexample.com%2Fsub%3Ffoo%3Dbar"), "https://example.com/sub?foo=bar");
        assert_eq!(ClipboardHelper::percent_decode("hello%20world"), "hello world");
        assert_eq!(ClipboardHelper::percent_decode("%E4%BD%A0%E5%A5%BD"), "你好");
    }
}
