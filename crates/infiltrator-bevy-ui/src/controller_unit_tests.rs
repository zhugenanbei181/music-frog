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
fn endpoint_parser_rejects_junk() {
    assert_eq!(parse_controller_endpoint(""), None);
    assert_eq!(parse_controller_endpoint("   "), None);
    assert_eq!(parse_controller_endpoint("127.0.0.1:9099"), None);
    assert_eq!(parse_controller_endpoint("http://"), None);
    assert_eq!(parse_controller_endpoint("https:///path"), None);
    assert_eq!(parse_controller_endpoint("ftp://host"), None);
}

#[test]
fn config_from_raw_toggles_demo_and_trims_secret() {
    assert!(controller_config_from_raw(None, None).is_none());
    assert!(controller_config_from_raw(Some("junk"), None).is_none());
    let config = controller_config_from_raw(Some("http://127.0.0.1:9099"), Some("  s3cr3t "))
        .expect("valid controller");
    assert_eq!(config.endpoint, "http://127.0.0.1:9099");
    assert_eq!(config.secret.as_deref(), Some("s3cr3t"));
    // An empty secret is no secret.
    let config = controller_config_from_raw(Some("http://127.0.0.1:9099"), Some("   "))
        .expect("valid controller");
    assert_eq!(config.secret, None);
}

#[test]
fn mode_wire_round_trip() {
    for mode in [ProxyMode::Rule, ProxyMode::Global, ProxyMode::Direct] {
        assert_eq!(ProxyMode::from_wire(mode.to_wire()), Some(mode));
    }
    assert_eq!(ProxyMode::from_wire(" GLOBAL "), Some(ProxyMode::Global));
    assert_eq!(ProxyMode::from_wire("meta"), None);
}

#[test]
fn connection_count_counts_the_table() {
    let mut snapshot = ConnectionsResponse::default();
    assert_eq!(active_connection_count(&snapshot), 0);
    snapshot.connections.push(mihomo_api::types::Connection {
        id: "a".to_owned(),
        metadata: mihomo_api::types::ConnectionMetadata::default(),
        upload: 0,
        download: 0,
        start: String::new(),
        rule: String::new(),
        rule_payload: String::new(),
        chains: vec![],
    });
    assert_eq!(active_connection_count(&snapshot), 1);
}

#[test]
fn rates_need_a_window_and_restart_after_counter_reset() {
    let now = Instant::now();
    // No previous window: honest zero.
    assert_eq!(rates_from_totals(None, 100, 200, now), (0.0, 0.0));
    let window = (1_000u64, 2_000u64, now - Duration::from_secs(2));
    let (up, down) = rates_from_totals(Some(window), 3_000, 6_000, now);
    assert_eq!(up, 1_000.0);
    assert_eq!(down, 2_000.0);
    // Counters went backwards (core restart): zero, never negative.
    let (up, down) = rates_from_totals(Some(window), 5, 5, now);
    assert_eq!(up, 0.0);
    assert_eq!(down, 0.0);
    // Zero-width window: zero, never a division blowup.
    let (up, _) = rates_from_totals(Some((0, 0, now)), 10, 10, now);
    assert_eq!(up, 0.0);
}

/// The exact payload the pinned v1.19.18 core serves on an idle
/// tracker: `"connections": null` must fold into an empty, honest
/// zero — not a decode failure.
#[test]
fn idle_connections_snapshot_tolerates_the_null_tracker() {
    let snapshot: IdleConnectionsSnapshot = serde_json::from_str(
        r#"{"downloadTotal":0,"uploadTotal":0,"connections":null,"memory":40529920}"#,
    )
    .expect("the idle shape parses");
    let response = ConnectionsResponse::from(snapshot);
    assert_eq!(active_connection_count(&response), 0);
    assert_eq!(response.download_total, 0);
    assert_eq!(response.upload_total, 0);

    // A populated tracker keeps the regular shape (the mihomo-api
    // primary read handles it; the fallback must not regress it).
    let snapshot: IdleConnectionsSnapshot = serde_json::from_str(
            r#"{"downloadTotal":7,"uploadTotal":9,"connections":[{"id":"a","metadata":{},"rule":"MATCH"}]}"#,
        )
        .expect("the populated shape parses");
    let response = ConnectionsResponse::from(snapshot);
    assert_eq!(active_connection_count(&response), 1);
    assert_eq!(response.upload_total, 9);
}
