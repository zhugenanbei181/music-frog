use super::*;
use crate::runtime::{Connection, ConnectionMetadata};
use chrono::TimeZone;
use lurker_core_contract::identity::{CoreGeneration, CoreIdentity, CoreInstanceId, EngineId};

fn scope(engine: &str) -> CoreScope {
    CoreScope {
        identity: CoreIdentity::new(
            EngineId::new(engine).expect("engine ID"),
            CoreInstanceId::new("active").expect("instance ID"),
            None,
        ),
        generation: CoreGeneration::new(7),
        session_token: None,
    }
}

fn connection(id: &str, start: &str) -> Connection {
    Connection {
        id: id.to_owned(),
        metadata: ConnectionMetadata {
            host: format!("{id}.example"),
            ..ConnectionMetadata::default()
        },
        upload: 17,
        download: 31,
        start: start.to_owned(),
        rule: "DOMAIN-SUFFIX".to_owned(),
        rule_payload: "example".to_owned(),
        chains: vec!["PROXY".to_owned(), "Hong Kong 01".to_owned()],
    }
}

fn fixed_duration(_start: &str, _observed_at: DateTime<Utc>) -> u64 {
    1_250
}

fn observation() -> ClosedConnectionObservation {
    ClosedConnectionObservation::new(
        Utc.with_ymd_and_hms(2026, 11, 12, 6, 0, 0).unwrap(),
        fixed_duration,
    )
}

fn revision(value: u64) -> SnapshotRevision {
    SnapshotRevision::new(value)
}

#[test]
fn first_observation_is_only_a_baseline() {
    let mut history = ClosedConnectionHistory::new(4);
    let connections = [connection("c-1", "2026-11-12T05:00:00Z")];

    history.observe(&scope("mihomo"), revision(1), &connections, observation());

    assert_eq!(
        history.phase,
        HistoryPhase::Tracking {
            scope: scope("mihomo"),
            revision: revision(1),
        }
    );
    assert_eq!(history.readback(), ClosedConnectionsSnapshot::default());
}

#[test]
fn a_disappeared_connection_is_read_back_with_its_last_facts() {
    let mut history = ClosedConnectionHistory::new(4);
    let scope = scope("mihomo");
    let first = [
        connection("c-1", "2026-11-12T05:00:00Z"),
        connection("c-2", "2026-11-12T05:00:00Z"),
    ];

    history.observe(&scope, revision(1), &first, observation());
    history.observe(&scope, revision(2), &first[1..], observation());

    let readback = history.readback();
    assert_eq!(readback.total_closed, 1);
    assert_eq!(
        readback.records,
        vec![ClosedConnectionSnapshot {
            id: "c-1".to_owned(),
            destination: "c-1.example".to_owned(),
            duration_ms: 1_250,
            upload_bytes: 17,
            download_bytes: 31,
            rule: "DOMAIN-SUFFIX".to_owned(),
            outbound: "PROXY → Hong Kong 01".to_owned(),
            closed_at: "2026-11-12T06:00:00+00:00".to_owned(),
        }]
    );
}

#[test]
fn a_foreign_scope_resets_the_stream_without_reporting_closures() {
    let mut history = ClosedConnectionHistory::new(4);
    let old_scope = scope("mihomo");
    let new_scope = scope("sing-box");
    let connections = [connection("c-1", "2026-11-12T05:00:00Z")];

    history.observe(&old_scope, revision(1), &connections, observation());
    history.observe(&new_scope, revision(1), &[] as &[Connection], observation());
    history.observe(&new_scope, revision(2), &connections, observation());
    history.observe(&new_scope, revision(3), &[] as &[Connection], observation());

    assert_eq!(history.readback().records.len(), 1);
    assert_eq!(history.readback().total_closed, 1);
    assert_eq!(history.readback().records[0].id, "c-1");
}

#[test]
fn eviction_keeps_the_newest_rows_and_the_lifetime_total() {
    let mut history = ClosedConnectionHistory::new(2);
    let scope = scope("mihomo");
    let mut ids = (0..3)
        .map(|index| connection(&format!("c-{index}"), "2026-11-12T05:00:00Z"))
        .collect::<Vec<_>>();

    history.observe(&scope, revision(1), &ids, observation());
    for current_revision in 2..=4 {
        ids.remove(0);
        history.observe(&scope, revision(current_revision), &ids, observation());
    }

    let readback = history.readback();
    assert_eq!(readback.total_closed, 3);
    assert_eq!(
        readback
            .records
            .iter()
            .map(|row| row.id.as_str())
            .collect::<Vec<_>>(),
        vec!["c-1", "c-2"]
    );
}

#[test]
fn clear_keeps_the_active_baseline_and_resets_the_lifetime_total() {
    let mut history = ClosedConnectionHistory::new(4);
    let scope = scope("mihomo");
    let connections = [connection("c-1", "2026-11-12T05:00:00Z")];

    history.observe(&scope, revision(1), &connections, observation());
    history.observe(&scope, revision(2), &[] as &[Connection], observation());
    assert_eq!(history.clear(), ClosedConnectionsSnapshot::default());
    history.observe(&scope, revision(3), &connections, observation());

    assert_eq!(history.readback(), ClosedConnectionsSnapshot::default());
}

#[test]
fn default_history_retains_the_shared_cap() {
    let mut history = ClosedConnectionHistory::default();
    let scope = scope("mihomo");
    let connections = (0..DEFAULT_CLOSED_CONNECTION_CAP + 1)
        .map(|index| connection(&format!("c-{index}"), "2026-11-12T05:00:00Z"))
        .collect::<Vec<_>>();

    history.observe(&scope, revision(1), &connections, observation());
    history.observe(&scope, revision(2), &[] as &[Connection], observation());

    assert_eq!(
        history.readback().records.len(),
        DEFAULT_CLOSED_CONNECTION_CAP
    );
    assert_eq!(
        history.readback().total_closed,
        DEFAULT_CLOSED_CONNECTION_CAP as u64 + 1
    );
}

#[test]
fn an_out_of_order_older_membership_revision_cannot_reopen_a_closed_row() {
    let mut history = ClosedConnectionHistory::new(4);
    let scope = scope("mihomo");
    let connections = [connection("c-1", "2026-11-12T05:00:00Z")];

    history.observe(&scope, revision(1), &connections, observation());
    history.observe(&scope, revision(2), &[] as &[Connection], observation());
    history.observe(&scope, revision(1), &connections, observation());

    assert_eq!(history.readback().total_closed, 1);
    assert_eq!(history.readback().records[0].id, "c-1");
}
