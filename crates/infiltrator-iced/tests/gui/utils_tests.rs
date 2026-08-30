//! `utils` unit tests (mounted from `src/utils.rs`): byte formatting and
//! redacted UI text (CORE-001).

use super::*;

#[test]
fn test_format_bytes() {
    assert_eq!(format_bytes(500), "500 B");
    assert_eq!(format_bytes(1024), "1.00 KB");
    assert_eq!(format_bytes(1024 * 1024), "1.00 MB");
    assert_eq!(format_bytes(1024 * 1024 * 1024), "1.00 GB");
    assert_eq!(format_bytes(1500 * 1024), "1.46 MB");
}

#[test]
fn test_sanitize_ui_text_masks_subscription_query_token() {
    let out = sanitize_ui_text("update failed: GET https://sub.example.com/d?token=tok1234");
    assert_eq!(
        out,
        "update failed: GET https://sub.example.com/d?token=***"
    );
}

#[test]
fn test_sanitize_ui_text_masks_secrets_userinfo_and_bearer() {
    let out = sanitize_ui_text("secret: abc123 dialing socks5://admin:pass123@10.0.0.1:1080");
    assert_eq!(out, "secret: *** dialing socks5://admin:***@10.0.0.1:1080");
    assert_eq!(
        sanitize_ui_text("Authorization: Bearer abc123"),
        "Authorization: Bearer ***"
    );
}

#[test]
fn test_sanitize_ui_text_preserves_plain_text() {
    let line = "core started, 12 proxies, rule provider reload done";
    assert_eq!(sanitize_ui_text(line), line);
}
