use super::*;

#[test]
fn endpoint_parser_accepts_http_urls_with_hosts() {
    assert_eq!(
        parse_controller_endpoint("http://127.0.0.1:9099"),
        Some("http://127.0.0.1:9099".to_owned())
    );
    assert_eq!(
        parse_controller_endpoint("  https://core.lan:9090/  "),
        Some("https://core.lan:9090/".to_owned())
    );
    assert_eq!(
        parse_controller_endpoint("http://localhost"),
        Some("http://localhost".to_owned())
    );
}

#[test]
fn endpoint_parser_rejects_non_http_or_missing_host_values() {
    for raw in [
        "",
        "   ",
        "127.0.0.1:9099",
        "http://",
        "https:///path",
        "ftp://host",
    ] {
        assert_eq!(parse_controller_endpoint(raw), None, "{raw:?}");
    }
}

#[test]
fn controller_config_trims_secret_and_keeps_demo_on_missing_controller() {
    assert!(controller_config_from_raw(None, None).is_none());
    assert!(controller_config_from_raw(Some("junk"), None).is_none());

    let config = controller_config_from_raw(Some("http://127.0.0.1:9099"), Some("  s3cr3t "))
        .expect("valid controller");
    assert_eq!(config.endpoint, "http://127.0.0.1:9099");
    assert_eq!(config.secret.as_deref(), Some("s3cr3t"));

    let config = controller_config_from_raw(Some("http://127.0.0.1:9099"), Some("   "))
        .expect("valid controller");
    assert_eq!(config.secret, None);
}
