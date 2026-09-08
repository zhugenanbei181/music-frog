//! Pure validation for Mihomo LAN access-control settings.

use ipnet::IpNet;
use std::collections::HashSet;
use std::str::FromStr;

const MAX_CIDR_ENTRIES: usize = 256;
const MAX_USERNAME_CHARS: usize = 128;
const MAX_PASSWORD_CHARS: usize = 512;

/// Normalize a user-entered CIDR list while preserving first-seen order.
/// Empty entries are ignored so comma/semicolon/newline-separated fields can
/// be edited naturally; malformed non-empty entries fail closed.
pub fn normalize_cidrs(values: &[String], field: &str) -> Result<Vec<String>, String> {
    if values.len() > MAX_CIDR_ENTRIES {
        return Err(format!(
            "{field} contains more than {MAX_CIDR_ENTRIES} CIDR entries"
        ));
    }
    let mut seen = HashSet::new();
    let mut normalized = Vec::new();
    for raw in values {
        let value = raw.trim();
        if value.is_empty() {
            continue;
        }
        let network = IpNet::from_str(value)
            .map_err(|_| format!("{field} contains an invalid CIDR: {value}"))?;
        let value = network.to_string();
        if seen.insert(value.clone()) {
            normalized.push(value);
        }
    }
    Ok(normalized)
}

/// Validate the single HTTP Basic Authentication credential accepted by the
/// current UI. Separators and control characters are rejected because
/// Mihomo stores users as `username:password` strings.
pub fn validate_credentials(username: &str, password: &str) -> Result<(), String> {
    let username = username.trim();
    if username.is_empty() {
        return Err("LAN authentication username cannot be empty".to_owned());
    }
    if username.chars().count() > MAX_USERNAME_CHARS {
        return Err("LAN authentication username is too long".to_owned());
    }
    if username.contains(':') || username.chars().any(|value| matches!(value, '\r' | '\n')) {
        return Err("LAN authentication username contains a forbidden separator".to_owned());
    }
    if password.is_empty() {
        return Err("LAN authentication password cannot be empty".to_owned());
    }
    if password.chars().count() > MAX_PASSWORD_CHARS {
        return Err("LAN authentication password is too long".to_owned());
    }
    if password.chars().any(|value| matches!(value, '\r' | '\n')) {
        return Err(
            "LAN authentication password contains a forbidden control character".to_owned(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cidrs_are_canonicalized_and_deduplicated() {
        let values = vec![
            "192.168.1.10/32".to_owned(),
            " 192.168.1.10/32 ".to_owned(),
            "10.0.0.0/8".to_owned(),
        ];
        assert_eq!(
            normalize_cidrs(&values, "allow").expect("valid CIDRs"),
            vec!["192.168.1.10/32", "10.0.0.0/8"]
        );
    }

    #[test]
    fn malformed_cidr_and_basic_auth_separators_fail_closed() {
        assert!(normalize_cidrs(&["192.168.1.0/33".to_owned()], "allow").is_err());
        assert!(validate_credentials("user:name", "password").is_err());
        assert!(validate_credentials("user", "line\nbreak").is_err());
        assert!(validate_credentials("user", "password").is_ok());
    }
}
