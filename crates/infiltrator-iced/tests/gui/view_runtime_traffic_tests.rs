use super::*;
use infiltrator_domain::connection_rate::{ConnectionRate, ConnectionRates};
use infiltrator_domain::runtime::{Connection, ConnectionMetadata};
use infiltrator_domain::traffic_audit::{TrafficDimension, TrafficWindow};
use infiltrator_domain::traffic_audit_readback::UNATTRIBUTED_PROCESS_ENTITY;

fn make_test_conn(
    id: &str,
    host: &str,
    source_ip: &str,
    process: &str,
    chains: Vec<String>,
) -> Connection {
    Connection {
        id: id.to_string(),
        metadata: ConnectionMetadata {
            network: "tcp".to_string(),
            connection_type: "TLS".to_string(),
            source_ip: source_ip.to_string(),
            destination_ip: "1.1.1.1".to_string(),
            source_port: "50000".to_string(),
            destination_port: "443".to_string(),
            host: host.to_string(),
            dns_mode: "fake-ip".to_string(),
            process_path: process.to_string(),
            special_proxy: String::new(),
            destination_geo_ip: None,
            destination_ip_asn: String::new(),
        },
        start: "2026-09-01T12:00:00Z".to_string(),
        rule: "Match".to_string(),
        rule_payload: String::new(),
        chains,
        ..Connection::default()
    }
}

fn rates_for(pairs: &[(&str, u64, u64)]) -> ConnectionRates {
    ConnectionRates::from_pairs(pairs.iter().map(|(id, up, down)| {
        (
            (*id).to_string(),
            ConnectionRate {
                upload_delta_bytes: *up,
                download_delta_bytes: *down,
                ..ConnectionRate::default()
            },
        )
    }))
}

/// Before the first observation there is no instant to cut a window against,
/// so the section has nothing to render rather than an invented empty window.
#[test]
fn the_section_has_no_readback_before_the_first_observation() {
    let (state, _) = AppState::new();
    assert_eq!(state.diag.traffic_audit.readback(AUDIT_RANK_LIMIT), None);
}

/// The default selection is the reference product's own (`host` over `24h`).
#[test]
fn the_default_selection_is_the_reference_default() {
    let (state, _) = AppState::new();
    assert_eq!(state.diag.traffic_audit.dimension, TrafficDimension::Domain);
    assert_eq!(state.diag.traffic_audit.window, TrafficWindow::LastDay);
}

/// The rows follow the reader's dimension, and every entity name is the owner's
/// attribution - the surface derives none of them.
#[test]
fn the_rows_follow_the_selected_dimension() {
    let (mut state, _) = AppState::new();
    let conns = vec![
        make_test_conn(
            "c1",
            "google.com",
            "10.0.0.1",
            "/bin/git",
            vec!["ProxyGroup".into()],
        ),
        make_test_conn("c2", "youtube.com", "10.0.0.2", "", vec!["DIRECT".into()]),
    ];
    let rates = rates_for(&[("c1", 100, 900), ("c2", 1_000, 3_000)]);
    state
        .diag
        .traffic_audit
        .observe(1_700_000_000, &conns, &rates);

    let domains = state
        .diag
        .traffic_audit
        .readback(AUDIT_RANK_LIMIT)
        .expect("observed");
    assert_eq!(domains.rows.len(), 2);
    assert_eq!(domains.rows[0].entity, "youtube.com");
    assert_eq!(domains.rows[0].total_bytes, 4_000);
    assert_eq!(domains.rows[0].share_percent, 80.0);
    assert_eq!(domains.totals.total_bytes(), 5_000);

    state.diag.traffic_audit.dimension = TrafficDimension::Device;
    let devices = state.diag.traffic_audit.readback(AUDIT_RANK_LIMIT).unwrap();
    assert_eq!(devices.rows[0].entity, "10.0.0.2");
    assert_eq!(devices.rows[0].total_bytes, 4_000);

    state.diag.traffic_audit.dimension = TrafficDimension::Node;
    let nodes = state.diag.traffic_audit.readback(AUDIT_RANK_LIMIT).unwrap();
    assert_eq!(nodes.rows[0].entity, "DIRECT");
    assert_eq!(nodes.rows[1].entity, "ProxyGroup");

    state.diag.traffic_audit.dimension = TrafficDimension::Process;
    let processes = state.diag.traffic_audit.readback(AUDIT_RANK_LIMIT).unwrap();
    assert_eq!(processes.rows[0].entity, UNATTRIBUTED_PROCESS_ENTITY);
    assert_eq!(processes.rows[1].entity, "git");
}

/// The selection the controls write is what the section reads back, and the
/// messages that carry it really move it.
#[test]
fn the_controls_move_the_selection_the_readback_answers() {
    let (mut state, _) = AppState::new();
    let conns = vec![make_test_conn(
        "c1",
        "google.com",
        "10.0.0.1",
        "/bin/git",
        vec!["ProxyGroup".into()],
    )];
    let rates = rates_for(&[("c1", 100, 900)]);
    state
        .diag
        .traffic_audit
        .observe(1_700_000_000, &conns, &rates);

    let _ = state.update(Message::SetTrafficAuditWindow(TrafficWindow::LastMonth));
    let _ = state.update(Message::SetTrafficAuditDimension(TrafficDimension::Process));

    let readback = state.diag.traffic_audit.readback(AUDIT_RANK_LIMIT).unwrap();
    assert_eq!(readback.window, TrafficWindow::LastMonth);
    assert_eq!(readback.dimension, TrafficDimension::Process);
    assert_eq!(readback.rows[0].entity, "git");
}

/// The range really cuts the window: the hour keeps only the recent flow while
/// the month still counts both.
#[test]
fn the_range_cuts_the_window_the_rows_and_the_trend_are_drawn_from() {
    let (mut state, _) = AppState::new();
    let conns = vec![make_test_conn(
        "c1",
        "google.com",
        "10.0.0.1",
        "/bin/git",
        vec!["ProxyGroup".into()],
    )];
    let recent = rates_for(&[("c1", 100, 900)]);
    let older = rates_for(&[("c1", 200, 800)]);
    let now = 86_400 * 20_000;
    state.diag.traffic_audit.observe(now - 60, &conns, &recent);
    state
        .diag
        .traffic_audit
        .observe(now - 3 * 86_400, &conns, &older);

    state.diag.traffic_audit.window = TrafficWindow::LastHour;
    let hour = state.diag.traffic_audit.readback(AUDIT_RANK_LIMIT).unwrap();
    assert_eq!(hour.totals.total_bytes(), 1_000);
    assert_eq!(hour.trend.len(), 1);

    state.diag.traffic_audit.window = TrafficWindow::LastMonth;
    let month = state.diag.traffic_audit.readback(AUDIT_RANK_LIMIT).unwrap();
    assert_eq!(month.totals.total_bytes(), 2_000);
    assert_eq!(month.rows[0].total_bytes, 2_000);
    assert!(month.trend.len() >= 2);
    assert_eq!(month.peak_row_total(), 2_000);
}
