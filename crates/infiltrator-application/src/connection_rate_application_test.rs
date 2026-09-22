//! Behavior tests for the application connection-rate seam (DUAL-13-10/12).

use super::*;
use infiltrator_domain::connection_rate::ConnectionRate;
use infiltrator_domain::runtime::{Connection, ConnectionMetadata};
use std::time::Duration;

fn connection(id: &str, host: &str, port: &str, up: u64, down: u64) -> Connection {
    Connection {
        id: id.to_string(),
        metadata: ConnectionMetadata {
            destination_port: port.to_string(),
            host: host.to_string(),
            process_path: "/usr/bin/git".to_string(),
            ..ConnectionMetadata::default()
        },
        upload: up,
        download: down,
        rule: "DomainSuffix".to_string(),
        chains: vec!["PROXY".to_string()],
        ..Connection::default()
    }
}

fn snapshot(connections: Vec<Connection>, up: u64, down: u64) -> ConnectionSnapshot {
    ConnectionSnapshot {
        download_total: down,
        upload_total: up,
        connections,
    }
}

#[test]
fn successive_observations_publish_real_bytes_per_second() {
    let base = Instant::now();
    let application = ConnectionRateApplication::new();

    let first = application.observe_at(
        base,
        &[connection("c1", "api.github.com", "443", 1_000, 2_000)],
    );
    assert_eq!(first.get("c1"), ConnectionRate::default());

    let second = application.observe_at(
        base + Duration::from_secs(2),
        &[connection("c1", "api.github.com", "443", 3_000, 10_000)],
    );
    assert_eq!(second.get("c1").upload_bps, 1_000.0);
    assert_eq!(second.get("c1").download_bps, 4_000.0);

    // A composition clone shares the window, so it derives the same rate.
    let clone = application.clone();
    let third = clone.observe_at(
        base + Duration::from_secs(2) + Duration::from_millis(500),
        &[connection("c1", "api.github.com", "443", 3_500, 12_000)],
    );
    assert_eq!(third.get("c1").upload_bps, 1_000.0);
    assert_eq!(third.get("c1").download_bps, 4_000.0);

    application.reset();
    let after_reset = application.observe_at(base, &[connection("c1", "a", "443", 9, 9)]);
    assert_eq!(after_reset.get("c1"), ConnectionRate::default());
}

#[test]
fn the_shared_read_model_carries_the_derived_rates() {
    let base = Instant::now();
    let application = ConnectionRateApplication::new();
    let connections = vec![
        connection("c1", "api.github.com", "443", 0, 0),
        connection("c2", "", "", 0, 0),
    ];
    application.observe_at(base, &connections);

    let next = vec![
        connection("c1", "api.github.com", "443", 2_000, 4_000),
        connection("c3", "new.example.com", "8443", 10, 20),
    ];
    let rates = application.observe_at(base + Duration::from_secs(2), &next);
    let page = connections_page_snapshot(&snapshot(next, 2_010, 4_020), &rates);

    assert_eq!(page.total_connections, 2);
    assert_eq!(page.total_upload_bytes, 2_010);
    assert_eq!(page.total_download_bytes, 4_020);

    let first = &page.connections[0];
    assert_eq!(first.id, "c1");
    assert_eq!(first.host, "api.github.com:443");
    assert_eq!(first.process, "/usr/bin/git");
    assert_eq!(first.upload_bps, 1_000.0);
    assert_eq!(first.download_bps, 2_000.0);
    assert_eq!(first.upload_total, 2_000);
    assert_eq!(first.download_total, 4_000);
    assert_eq!(first.chains, vec!["PROXY".to_string()]);

    // The freshly observed connection has no previous window: honest zero,
    // never a fabricated burst, and the host falls back to the empty host.
    let second = &page.connections[1];
    assert_eq!(second.id, "c3");
    assert_eq!(second.upload_bps, 0.0);
    assert_eq!(second.download_bps, 0.0);
    assert_eq!(second.host, "new.example.com:8443");

    // A connection the controller no longer reports disappears from the page
    // even though the book still has no entry for it.
    assert_eq!(page.connections.len(), 2);
    assert_eq!(rates.get("c2"), ConnectionRate::default());
    assert_eq!(rates.len(), 2);
}
