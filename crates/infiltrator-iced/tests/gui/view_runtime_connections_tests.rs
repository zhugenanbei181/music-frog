use super::*;
use infiltrator_domain::connection_rate::{ConnectionRate, ConnectionRates};
use infiltrator_domain::runtime::{Connection, ConnectionMetadata};

fn make_test_conn(id: &str, host: &str, process: &str, up: u64, down: u64) -> Connection {
    Connection {
        id: id.to_string(),
        metadata: ConnectionMetadata {
            network: "tcp".to_string(),
            connection_type: "TLS".to_string(),
            source_ip: "192.168.1.50".to_string(),
            destination_ip: "1.1.1.1".to_string(),
            source_port: "50000".to_string(),
            destination_port: "443".to_string(),
            host: host.to_string(),
            dns_mode: "fake-ip".to_string(),
            process_path: process.to_string(),
            special_proxy: String::new(),
        },
        upload: up,
        download: down,
        start: "2026-09-01T12:00:00Z".to_string(),
        rule: "DomainSuffix".to_string(),
        rule_payload: "example.com".to_string(),
        chains: vec!["DMIT".to_string(), "PROXY".to_string()],
    }
}

#[test]
fn test_extract_process_name() {
    assert_eq!(extract_process_name("/usr/bin/firefox"), "firefox");
    assert_eq!(
        extract_process_name("C:\\Program Files\\Zed\\zed-editor.exe"),
        "zed-editor"
    );
    assert_eq!(extract_process_name("zed-editor"), "zed-editor");
    assert_eq!(extract_process_name(""), "");
    assert_eq!(extract_process_name("   "), "");
}

#[test]
fn test_outbound_target_info() {
    let mut conn = make_test_conn("1", "google.com", "", 100, 200);
    let (target, kind) = outbound_target_info(&conn);
    assert_eq!(target, "DMIT");
    assert_eq!(kind, BadgeKind::Accent);

    conn.chains = vec!["DIRECT".to_string()];
    let (target, kind) = outbound_target_info(&conn);
    assert_eq!(target, "DIRECT");
    assert_eq!(kind, BadgeKind::Success);

    conn.chains = vec!["REJECT".to_string()];
    let (target, kind) = outbound_target_info(&conn);
    assert_eq!(target, "REJECT");
    assert_eq!(kind, BadgeKind::Danger);
}

#[test]
fn test_filter_connection() {
    let conn = make_test_conn("c1", "api.openai.com", "/usr/bin/chromium", 100, 200);
    assert!(filter_connection(&conn, ""));
    assert!(filter_connection(&conn, "openai"));
    assert!(filter_connection(&conn, "chromium"));
    assert!(filter_connection(&conn, "DMIT"));
    assert!(!filter_connection(&conn, "nonexistent"));
}

#[test]
fn test_sort_connections() {
    let mut conns = vec![
        make_test_conn("1", "b.com", "", 100, 500),
        make_test_conn("2", "a.com", "", 900, 200),
        make_test_conn("3", "c.com", "", 500, 800),
    ];

    // Cumulative keys keep the original ordering through the shared reduction.
    let empty = ConnectionRates::default();
    let rows = sort_rated_connections(&conns, &empty, "download_desc");
    assert_eq!(
        rows.iter()
            .map(|row| row.row.id.as_str())
            .collect::<Vec<_>>(),
        vec!["3", "1", "2"]
    );
    let rows = sort_rated_connections(&conns, &empty, "upload_desc");
    assert_eq!(
        rows.iter()
            .map(|row| row.row.id.as_str())
            .collect::<Vec<_>>(),
        vec!["2", "3", "1"]
    );
    let rows = sort_rated_connections(&conns, &empty, "host_asc");
    assert_eq!(
        rows.iter()
            .map(|row| row.row.id.as_str())
            .collect::<Vec<_>>(),
        vec!["2", "1", "3"]
    );

    // DUAL-13-12: the rate keys rank on the derived instantaneous rates, not
    // on the cumulative totals above.
    let rates = ConnectionRates::from_pairs([
        (
            "1".to_string(),
            ConnectionRate {
                upload_bps: 0.0,
                download_bps: 9_000.0,
            },
        ),
        (
            "2".to_string(),
            ConnectionRate {
                upload_bps: 5_000.0,
                download_bps: 0.0,
            },
        ),
        (
            "3".to_string(),
            ConnectionRate {
                upload_bps: 1_000.0,
                download_bps: 1_000.0,
            },
        ),
    ]);
    let rows = sort_rated_connections(&conns, &rates, "download_rate_desc");
    assert_eq!(
        rows.iter()
            .map(|row| row.row.id.as_str())
            .collect::<Vec<_>>(),
        vec!["1", "3", "2"]
    );
    let rows = sort_rated_connections(&conns, &rates, "upload_rate_desc");
    assert_eq!(
        rows.iter()
            .map(|row| row.row.id.as_str())
            .collect::<Vec<_>>(),
        vec!["2", "3", "1"]
    );

    // The windowed view slices the rated rows, so the bounds still line up.
    conns.truncate(2);
    let rows = sort_rated_connections(&conns, &rates, "download_rate_desc");
    assert_eq!(rows.len(), 2);
}

#[test]
fn test_high_throughput_pulse_uses_the_shared_threshold() {
    let slow = make_test_conn("slow", "slow.example.com", "/usr/bin/curl", 0, 0);
    let fast = make_test_conn("fast", "fast.example.com", "/usr/bin/curl", 0, 0);
    let threshold = infiltrator_domain::connection_rate::HIGH_THROUGHPUT_THRESHOLD_BPS;
    let rates = ConnectionRates::from_pairs([
        (
            "slow".to_string(),
            ConnectionRate {
                upload_bps: threshold - 1.0,
                download_bps: 1_000.0,
            },
        ),
        (
            "fast".to_string(),
            ConnectionRate {
                upload_bps: threshold,
                download_bps: 0.0,
            },
        ),
    ]);

    // Below the threshold: no glow at any phase.
    assert_eq!(connection_pulse_intensity(&slow, &rates, 0.5), 0.0);
    // At/above the threshold: the shared breathing intensity, deterministic
    // for a given phase and strongest at the mid-breath.
    let low = connection_pulse_intensity(&fast, &rates, 0.0);
    let peak = connection_pulse_intensity(&fast, &rates, 0.5);
    assert_eq!(
        low,
        infiltrator_domain::connection_rate::PULSE_MIN_INTENSITY
    );
    assert!(peak > low);
    assert!((peak - infiltrator_domain::connection_rate::PULSE_MAX_INTENSITY).abs() < 1e-5);
    // A connection with no observation window shows no glow.
    let unknown = make_test_conn("unknown", "x.example.com", "/bin/x", 0, 0);
    assert_eq!(connection_pulse_intensity(&unknown, &rates, 0.5), 0.0);
}

#[test]
fn test_stream_badge_kinds() {
    let _elem_idle: Element<'_, Message> = stream_badge(&RuntimeStreamState::Idle, &Lang("zh-CN"));
    let _elem_connected: Element<'_, Message> =
        stream_badge(&RuntimeStreamState::Connected, &Lang("zh-CN"));
    let _elem_failed: Element<'_, Message> =
        stream_badge(&RuntimeStreamState::Failed("err".into()), &Lang("zh-CN"));
}

#[test]
fn test_shared_connection_view_reductions_are_delegated() {
    use infiltrator_domain::connection_view::{ConnectionGroupingMode, quick_rule_spec};

    let conns = vec![
        make_test_conn("1", "b.com", "/usr/bin/git", 100, 500),
        make_test_conn("2", "a.com", "/usr/bin/curl", 900, 200),
    ];

    // Search (DUAL-13-13) and sort resolve through the shared domain.
    assert!(filter_connection(&conns[0], "git"));
    assert!(!filter_connection(&conns[0], "curl"));
    let rows = sort_rated_connections(&conns, &ConnectionRates::default(), "upload_desc");
    assert_eq!(rows[0].row.id, "2");

    // Aggregation (DUAL-13-02) is the shared bucket reduction.
    let buckets = infiltrator_domain::connection_view::aggregate_connections(
        &conns,
        ConnectionGroupingMode::ByProcess,
    );
    assert_eq!(buckets.len(), 2);
    assert_eq!(buckets[0].key, "curl");
    assert_eq!(buckets[0].upload_total, 900);

    // Reverse rule draft (DUAL-13-09) uses the bare host, not host:port.
    let spec = quick_rule_spec(rows[0].row, "DIRECT");
    assert_eq!(spec.pattern, "DOMAIN-SUFFIX,a.com");
    assert_eq!(spec.rule_line(), "DOMAIN-SUFFIX,a.com,DIRECT");
}

#[test]
fn test_stream_state_maps_to_shared_phase() {
    use infiltrator_contract::connection::ConnectionStreamPhase;

    assert_eq!(
        RuntimeStreamState::Idle.shared_phase(),
        ConnectionStreamPhase::Idle
    );
    assert_eq!(
        RuntimeStreamState::Connecting.shared_phase(),
        ConnectionStreamPhase::Connecting
    );
    assert_eq!(
        RuntimeStreamState::Connected.shared_phase(),
        ConnectionStreamPhase::Live
    );
    assert_eq!(
        RuntimeStreamState::Reconnecting.shared_phase(),
        ConnectionStreamPhase::Reconnecting
    );
    assert_eq!(
        RuntimeStreamState::Failed("closed".to_string()).shared_phase(),
        ConnectionStreamPhase::Unavailable
    );
}

#[test]
fn test_route_chain_hops_parse_through_shared_model() {
    let conn = make_test_conn("1", "google.com", "/usr/bin/chrome", 0, 0);
    let chain = infiltrator_domain::connection_view::route_chain(&conn);
    assert_eq!(chain.hops(), ["DMIT", "PROXY"]);
    assert_eq!(chain.first(), Some("DMIT"));
    assert_eq!(chain.display(" → "), "DMIT → PROXY");
}
