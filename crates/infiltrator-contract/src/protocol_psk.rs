//! Shadowsocks-2022 PSK validation (DUAL-05-01), split out of
//! `protocol_fidelity.rs` to respect the business-file line budget.

use crate::protocol_fidelity::ProtocolIssue;

/// Validate a Shadowsocks-2022 PSK: one base64 key, or `key:key` for the
/// multi-user form, each decoding to the cipher's key size.
pub(crate) fn check_2022_psk(password: &str, key_bytes: usize, issues: &mut Vec<ProtocolIssue>) {
    if password.is_empty() {
        return;
    }
    for part in password.split(':') {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            issues.push(ProtocolIssue::new(
                "password",
                "2022 PSK components must not be empty",
            ));
            return;
        }
        if !trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=')
        {
            issues.push(ProtocolIssue::new(
                "password",
                "2022 PSK must be standard base64 (one key, or key:key for multi-user)",
            ));
            return;
        }
        let padding = trimmed.chars().filter(|c| *c == '=').count();
        let decoded = (trimmed.len() * 3 / 4).saturating_sub(padding);
        if decoded != key_bytes {
            issues.push(ProtocolIssue::new(
                "password",
                "2022 PSK length does not match the cipher key size",
            ));
            return;
        }
    }
}
