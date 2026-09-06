//! Pure validation for Windows AppContainer identifiers.

/// Validate and trim an AppContainer SID before it reaches a host command.
pub fn validate_app_container_sid(value: &str) -> Result<&str, String> {
    let sid = value.trim();
    if sid.is_empty() {
        return Err("AppContainer SID is empty".to_owned());
    }
    if sid.len() > 256 {
        return Err("AppContainer SID is too long".to_owned());
    }
    let suffix = sid.strip_prefix("S-1-").unwrap_or_default();
    if suffix.is_empty()
        || suffix
            .split('-')
            .any(|part| part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return Err(format!("invalid AppContainer SID: {sid}"));
    }
    Ok(sid)
}

#[cfg(test)]
mod tests {
    use super::validate_app_container_sid;

    #[test]
    fn app_container_sid_validation_accepts_real_shape() {
        assert_eq!(
            validate_app_container_sid(" S-1-15-2-1234 ").expect("valid SID"),
            "S-1-15-2-1234"
        );
    }

    #[test]
    fn app_container_sid_validation_rejects_injection() {
        assert!(validate_app_container_sid("S-1-15-2-1 & whoami").is_err());
        assert!(validate_app_container_sid(" ").is_err());
    }
}
