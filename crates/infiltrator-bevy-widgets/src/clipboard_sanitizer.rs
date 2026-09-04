//! Clipboard sanitization, invisible zero-width character stripping, and credential masking.
//!
//! Charter (docs/BEVY_UI_FRONTEND.md):
//! Pure sanitization pipeline ensuring pasted subscription links, tokens, and YAML configs
//! are stripped of hostile zero-width characters and credentials are masked for UI presentation.

/// Invisible zero-width codepoints that frequently pollute copy-pasted text.
pub const INVISIBLE_ZERO_WIDTH_CHARS: &[char] = &[
    '\u{200B}', // Zero-width space
    '\u{200C}', // Zero-width non-joiner
    '\u{200D}', // Zero-width joiner
    '\u{2060}', // Word joiner
    '\u{FEFF}', // Zero-width no-break space / byte order mark
];

/// Known proxy protocol URI scheme prefixes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProxyUriScheme {
    Vmess,
    Vless,
    Shadowsocks,
    Trojan,
    Hysteria2,
    Tuic,
    Http,
    Https,
    Clash,
}

/// Clean up pasted input by removing invisible zero-width spaces and normalizing line endings.
pub fn sanitize_pasted_text(raw: &str) -> String {
    raw.chars()
        .filter(|c| !INVISIBLE_ZERO_WIDTH_CHARS.contains(c))
        .filter(|&c| c != '\r') // Strip carriage return, normalize to \n
        .collect()
}

/// Mask sensitive token or secret string for safe UI presentation (e.g. `abcd...1234`).
pub fn mask_sensitive_token(secret: &str) -> String {
    let trimmed = secret.trim();
    let char_count = trimmed.chars().count();
    if char_count <= 8 {
        "********".to_string()
    } else {
        let prefix: String = trimmed.chars().take(4).collect();
        let suffix: String = trimmed.chars().skip(char_count - 4).collect();
        format!("{}...{}", prefix, suffix)
    }
}

/// Redact credentials embedded inside subscription or proxy URLs.
pub fn sanitize_url_credentials(url: &str) -> String {
    let mut result = url.to_string();

    // Redact password in user:password@
    if let Some(scheme_idx) = result.find("://") {
        let after_scheme = &result[scheme_idx + 3..];
        if let Some(at_idx) = after_scheme.find('@') {
            let auth = &after_scheme[..at_idx];
            if let Some(colon_idx) = auth.find(':') {
                let prefix = &result[..scheme_idx + 3 + colon_idx + 1];
                let suffix = &after_scheme[at_idx..];
                result = format!("{}***{}", prefix, suffix);
            }
        }
    }

    // Redact query parameter secrets: ?secret=... or &token=...
    for param in &["secret=", "token=", "password="] {
        if let Some(start) = result.find(param) {
            let val_start = start + param.len();
            let val_end = result[val_start..]
                .find('&')
                .map(|pos| val_start + pos)
                .unwrap_or(result.len());
            result.replace_range(val_start..val_end, "***");
        }
    }

    result
}

/// Detect proxy protocol scheme from URI string.
pub fn detect_uri_scheme(uri: &str) -> Option<ProxyUriScheme> {
    let lower = uri.trim().to_lowercase();
    if lower.starts_with("vmess://") {
        Some(ProxyUriScheme::Vmess)
    } else if lower.starts_with("vless://") {
        Some(ProxyUriScheme::Vless)
    } else if lower.starts_with("ss://") {
        Some(ProxyUriScheme::Shadowsocks)
    } else if lower.starts_with("trojan://") {
        Some(ProxyUriScheme::Trojan)
    } else if lower.starts_with("hysteria2://") || lower.starts_with("hy2://") {
        Some(ProxyUriScheme::Hysteria2)
    } else if lower.starts_with("tuic://") {
        Some(ProxyUriScheme::Tuic)
    } else if lower.starts_with("clash://") {
        Some(ProxyUriScheme::Clash)
    } else if lower.starts_with("https://") {
        Some(ProxyUriScheme::Https)
    } else if lower.starts_with("http://") {
        Some(ProxyUriScheme::Http)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zero_width_characters_stripping() {
        let dirty = "https://example.com/sub\u{200B}?token=\u{FEFF}abc\r\n";
        let cleaned = sanitize_pasted_text(dirty);
        assert_eq!(cleaned, "https://example.com/sub?token=abc\n");
    }

    #[test]
    fn test_mask_sensitive_token() {
        assert_eq!(mask_sensitive_token("short"), "********");
        assert_eq!(mask_sensitive_token("1234567890abcdef"), "1234...cdef");
    }

    #[test]
    fn test_sanitize_url_credentials() {
        let url1 = "https://admin:super_secret_password@proxy.lan:8080";
        assert_eq!(
            sanitize_url_credentials(url1),
            "https://admin:***@proxy.lan:8080"
        );

        let url2 = "https://sub.lan/get?secret=my_secret_key&format=yaml";
        assert_eq!(
            sanitize_url_credentials(url2),
            "https://sub.lan/get?secret=***&format=yaml"
        );
    }

    #[test]
    fn test_detect_uri_scheme() {
        assert_eq!(
            detect_uri_scheme("vmess://eyJhZGQi..."),
            Some(ProxyUriScheme::Vmess)
        );
        assert_eq!(
            detect_uri_scheme("hy2://password@server:443"),
            Some(ProxyUriScheme::Hysteria2)
        );
        assert_eq!(detect_uri_scheme("not_a_valid_url"), None);
    }
}
