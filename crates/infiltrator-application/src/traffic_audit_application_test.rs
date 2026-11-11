use super::*;
use infiltrator_domain::connection_rate::{ConnectionRate, ConnectionRateDiffer};
use infiltrator_domain::runtime::{Connection, ConnectionMetadata};
use std::slice::from_ref;
use std::time::{Duration, Instant};

fn connection(id: &str, host: &str, source_ip: &str, process_path: &str) -> Connection {
    Connection {
        id: id.to_string(),
        metadata: ConnectionMetadata {
            host: host.to_string(),
            source_ip: source_ip.to_string(),
            process_path: process_path.to_string(),
            ..ConnectionMetadata::default()
        },
        rule: "Match".to_string(),
        rule_payload: "漏网之鱼".to_string(),
        chains: vec!["HK-01".to_string(), "PROXIES".to_string()],
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

// The anchor instant is a day boundary, so every window's grid lines up.
fn now() -> u64 {
    86_400 * 20_000
}

// The audit records what the rate book measured, attributed to the entities the
// connection itself names - and a connection with no measured change records
// nothing at all.
#[test]
fn one_observation_is_recorded_once_per_named_dimension() {
    let audit = TrafficAuditApplication::new();
    let conns = vec![
        connection("c1", "a.com", "192.168.1.23", "/usr/bin/chromium"),
        connection("c2", "b.com", "192.168.1.23", ""),
        connection("c3", "c.com", "192.168.1.23", "/usr/bin/curl"),
    ];
    // c3 carried no change in this window.
    let rates = rates_for(&[("c1", 1_000, 2_000), ("c2", 500, 500), ("c3", 0, 0)]);
    audit.observe(now() - 60, &conns, &rates);

    let domains = audit.readback(TrafficDimension::Domain, TrafficWindow::LastHour, now(), 10);
    assert_eq!(domains.rows.len(), 2);
    assert_eq!(domains.rows[0].entity, "a.com");
    assert_eq!(domains.rows[0].total_bytes, 3_000);
    assert_eq!(domains.rows[1].entity, "b.com");
    assert_eq!(domains.rows[1].total_bytes, 1_000);
    assert_eq!(domains.totals.total_bytes(), 4_000);
    assert!((domains.rows[0].share_percent - 75.0).abs() < 1e-9);

    let devices = audit.readback(TrafficDimension::Device, TrafficWindow::LastHour, now(), 10);
    assert_eq!(devices.rows.len(), 1);
    assert_eq!(devices.rows[0].entity, "192.168.1.23");
    assert_eq!(devices.rows[0].total_bytes, 4_000);

    let nodes = audit.readback(TrafficDimension::Node, TrafficWindow::LastHour, now(), 10);
    assert_eq!(nodes.rows.len(), 1);
    assert_eq!(nodes.rows[0].entity, "HK-01");

    // The process that the core never named still owns its row.
    let processes = audit.readback(
        TrafficDimension::Process,
        TrafficWindow::LastHour,
        now(),
        10,
    );
    assert_eq!(processes.rows.len(), 2);
    assert_eq!(processes.rows[0].entity, "chromium");
    assert_eq!(processes.rows[0].total_bytes, 3_000);
    let unknown = processes
        .rows
        .iter()
        .find(|row| row.entity == UNATTRIBUTED_PROCESS_ENTITY)
        .expect("the unnamed process keeps its own row");
    assert_eq!(unknown.total_bytes, 1_000);
    // c3 moved no bytes in this window, so it is not a row at all: a zero-byte
    // rank would claim the process used the link when it did not.
    assert!(processes.rows.iter().all(|row| row.entity != "curl"));
}

// A host the core did not resolve is attributed to the address it reached, not
// dropped: the traffic happened either way.
#[test]
fn an_unresolved_host_is_attributed_to_its_address() {
    let audit = TrafficAuditApplication::new();
    let mut conn = connection("c1", "", "192.168.1.23", "/usr/bin/chromium");
    conn.metadata.destination_ip = "203.0.113.44".to_string();
    audit.observe(now() - 30, &[conn], &rates_for(&[("c1", 10, 20)]));

    let domains = audit.readback(TrafficDimension::Domain, TrafficWindow::LastHour, now(), 10);
    assert_eq!(domains.rows[0].entity, "203.0.113.44");
}

// The windows are the owner's: the same flow lands in the hour, the day, the
// week and the month, while a flow older than the month lands in none.
#[test]
fn the_readback_answers_only_the_window_it_was_asked_for() {
    let audit = TrafficAuditApplication::new();
    let conns = vec![connection("c1", "a.com", "192.168.1.23", "/usr/bin/curl")];
    let rates = rates_for(&[("c1", 1_000, 1_000)]);
    audit.observe(now() - 60, &conns, &rates);
    audit.observe(now() - 40 * 86_400, &conns, &rates);

    for window in [
        TrafficWindow::LastHour,
        TrafficWindow::LastDay,
        TrafficWindow::LastWeek,
        TrafficWindow::LastMonth,
    ] {
        let readback = audit.readback(TrafficDimension::Domain, window, now(), 10);
        assert_eq!(readback.window, window);
        assert_eq!(readback.dimension, TrafficDimension::Domain);
        assert_eq!(readback.totals.total_bytes(), 2_000, "{window:?}");
        assert_eq!(readback.rows[0].total_bytes, 2_000, "{window:?}");
    }

    // The fine tier keeps the last hour only, so once it is pruned the hour
    // window is honestly empty while the coarser windows still answer.
    audit.observe(now() + 3_600, &conns, &rates);
    let hour = audit.readback(
        TrafficDimension::Domain,
        TrafficWindow::LastHour,
        now() + 3_600,
        10,
    );
    assert_eq!(hour.rows.len(), 1);
    assert_eq!(hour.rows[0].total_bytes, 2_000);
    let day = audit.readback(
        TrafficDimension::Domain,
        TrafficWindow::LastDay,
        now() + 3_600,
        10,
    );
    assert_eq!(day.totals.total_bytes(), 4_000);
}

// Clearing drops every window, and the trend of a cleared audit is empty.
#[test]
fn clearing_the_audit_leaves_no_window_behind() {
    let audit = TrafficAuditApplication::new();
    let conns = vec![connection("c1", "a.com", "192.168.1.23", "/usr/bin/curl")];
    audit.observe(now() - 60, &conns, &rates_for(&[("c1", 100, 200)]));
    assert_eq!(
        audit
            .readback(TrafficDimension::Domain, TrafficWindow::LastHour, now(), 10)
            .totals
            .total_bytes(),
        300
    );

    audit.clear();
    let cleared = audit.readback(TrafficDimension::Domain, TrafficWindow::LastHour, now(), 10);
    assert!(cleared.rows.is_empty());
    assert!(cleared.trend.is_empty());
    assert_eq!(cleared.peak_row_total(), 0);
}

// End to end: the same differ the surfaces use turns two observations into the
// byte delta the audit records, so no second counter diff exists.
#[test]
fn the_delta_recorded_is_the_one_the_rate_differ_measured() {
    let audit = TrafficAuditApplication::new();
    let mut differ = ConnectionRateDiffer::new();
    let mut conn = connection("c1", "a.com", "192.168.1.23", "/usr/bin/curl");
    conn.upload = 0;
    conn.download = 0;

    let base = Instant::now();
    let first = differ.observe(base, from_ref(&conn));
    audit.observe(now() - 120, from_ref(&conn), &first);

    conn.upload = 4_096;
    conn.download = 8_192;
    let second = differ.observe(base + Duration::from_secs(2), from_ref(&conn));
    audit.observe(now() - 60, from_ref(&conn), &second);

    let readback = audit.readback(TrafficDimension::Domain, TrafficWindow::LastHour, now(), 10);
    assert_eq!(readback.totals.upload_bytes, 4_096);
    assert_eq!(readback.totals.download_bytes, 8_192);
    assert_eq!(
        readback
            .trend
            .iter()
            .map(|point| point.total_bytes())
            .sum::<u64>(),
        12_288
    );
}
