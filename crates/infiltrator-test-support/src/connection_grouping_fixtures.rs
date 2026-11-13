//! Deterministic connection records shared by application and surface tests.
use chrono::{DateTime, Utc};
use infiltrator_domain::connection_observation::{
    ConnectionObservationInstant, ObservedConnections,
};
use infiltrator_domain::runtime::{Connection, ConnectionMetadata, ConnectionSnapshot};

/// The instant the deterministic fixture was observed at.
///
/// The fixture is observed at a fixed instant, so every captured frame reads
/// the same age. A wall clock here would make the receipt unreproducible.
pub const GROUPING_OBSERVED_AT: &str = "2026-10-06T01:00:00Z";

/// The start of every fixture row, one per row.
///
/// The rows are spread across the second, minute and hour ladder, so a
/// surface that publishes an age shows every unit it can print instead of
/// one repeated number.
const GROUPING_STARTS: [&str; 8] = [
    "2026-10-05T01:00:00Z",
    "2026-10-05T20:00:00Z",
    "2026-10-05T23:00:00Z",
    "2026-10-06T00:30:00Z",
    "2026-10-06T00:55:00Z",
    "2026-10-06T00:58:00Z",
    "2026-10-06T00:59:00Z",
    "2026-10-06T00:59:55Z",
];

pub fn grouping_snapshot() -> ConnectionSnapshot {
    let mut connections: Vec<Connection> = GROUPING_STARTS
        .iter()
        .enumerate()
        .map(|(index, start)| Connection {
            id: format!("group-{index}"),
            start: (*start).to_owned(),
            metadata: ConnectionMetadata {
                host: format!("api-{index}.example.org"),
                destination_port: "443".into(),
                process_path: format!("/usr/bin/client-{index}"),
                network: "tcp".into(),
                destination_ip: format!("203.0.113.{}", index + 1),
                source_ip: "192.0.2.5".into(),
                source_port: "51000".into(),
                ..Default::default()
            },
            upload: (index as u64 + 1) * 1024,
            download: (index as u64 + 1) * 2048,
            ..Default::default()
        })
        .collect();
    let mut duplicate = connections[0].clone();
    duplicate.id = "group-duplicate".into();
    connections.push(duplicate);
    ConnectionSnapshot {
        upload_total: connections.iter().map(|row| row.upload).sum(),
        download_total: connections.iter().map(|row| row.download).sum(),
        connections,
    }
}

/// The deterministic fixture with the instant it was observed at.
pub fn grouping_observation() -> ObservedConnections {
    ObservedConnections::new(grouping_snapshot(), grouping_instant())
}

/// The instant the deterministic fixture was observed at.
pub fn grouping_instant() -> ConnectionObservationInstant {
    ConnectionObservationInstant::observed_at(
        DateTime::parse_from_rfc3339(GROUPING_OBSERVED_AT)
            .expect("fixture instant")
            .with_timezone(&Utc),
    )
}

/// Pair one snapshot with the fixture instant.
///
/// A surface test that builds its own rows still observes them at the
/// fixture instant, so the ages it renders stay deterministic.
pub fn observed(snapshot: ConnectionSnapshot) -> ObservedConnections {
    ObservedConnections::new(snapshot, grouping_instant())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fixture_rows_span_every_duration_unit() {
        let instant = grouping_instant();
        let ages: Vec<Option<u64>> = grouping_snapshot()
            .connections
            .iter()
            .map(|row| instant.age_of(&row.start))
            .collect();
        assert_eq!(
            ages,
            vec![
                Some(86_400_000),
                Some(18_000_000),
                Some(7_200_000),
                Some(1_800_000),
                Some(300_000),
                Some(120_000),
                Some(60_000),
                Some(5_000),
                Some(86_400_000),
            ]
        );
    }
}
