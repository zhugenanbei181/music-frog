use super::*;
use chrono::{TimeZone, Utc};
use infiltrator_domain::runtime::{Connection, ConnectionMetadata, ConnectionSnapshot};
use lurker_core_contract::identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, EngineId,
};
use lurker_core_contract::outbound::identity::SnapshotRevision;

fn scope() -> CoreScope {
    CoreScope {
        identity: CoreIdentity::new(
            EngineId::new("mihomo").expect("engine ID"),
            CoreInstanceId::new("active").expect("instance ID"),
            None,
        ),
        generation: CoreGeneration::new(7),
        session_token: None,
    }
}

fn observed_at() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 12, 5, 0, 1).unwrap()
}

fn connection(id: &str) -> Connection {
    Connection {
        id: id.to_owned(),
        metadata: ConnectionMetadata {
            host: format!("{id}.example"),
            ..ConnectionMetadata::default()
        },
        start: "2026-11-12T05:00:00+00:00".to_owned(),
        rule: "DOMAIN-SUFFIX".to_owned(),
        chains: vec!["PROXY".to_owned()],
        ..Default::default()
    }
}

fn observation(revision: u64, connections: Vec<Connection>) -> ConnectionObservationSnapshot {
    ConnectionObservationSnapshot::new(
        scope(),
        SnapshotRevision::new(revision),
        ConnectionSnapshot {
            connections,
            ..Default::default()
        },
    )
    .expect("valid connection observation")
}

#[test]
fn clones_share_one_history_and_read_back_the_disappeared_row() {
    let owner = ClosedConnectionsApplication::new();
    let shared = owner.clone();

    owner.observe_at(
        &observation(1, vec![connection("c-1"), connection("c-2")]),
        observed_at(),
    );
    owner.observe_at(&observation(2, vec![connection("c-2")]), observed_at());

    let readback = shared.readback();
    assert_eq!(readback.total_closed, 1);
    assert_eq!(readback.records[0].id, "c-1");
    assert_eq!(readback.records[0].destination, "c-1.example");
    assert_eq!(readback.records[0].outbound, "PROXY");
    assert_eq!(readback.records[0].duration_ms, 1_000);
    assert_eq!(readback.records[0].closed_at, "2026-11-12T05:00:01+00:00");
}

#[test]
fn an_older_membership_revision_cannot_reopen_a_closed_row() {
    let owner = ClosedConnectionsApplication::new();

    owner.observe_at(&observation(1, vec![connection("c-1")]), observed_at());
    owner.observe_at(&observation(2, vec![]), observed_at());
    owner.observe_at(&observation(1, vec![connection("c-1")]), observed_at());

    assert_eq!(owner.readback().total_closed, 1);
}

#[test]
fn a_scope_switch_starts_a_new_baseline_and_forges_no_closures() {
    let owner = ClosedConnectionsApplication::new();
    owner.observe_at(&observation(1, vec![connection("c-1")]), observed_at());
    let other_engine = ConnectionObservationSnapshot::new(
        CoreScope {
            identity: CoreIdentity::new(
                EngineId::new("sing-box").expect("engine ID"),
                CoreInstanceId::new("active").expect("instance ID"),
                None,
            ),
            generation: CoreGeneration::new(7),
            session_token: None,
        },
        SnapshotRevision::new(1),
        ConnectionSnapshot::default(),
    )
    .expect("valid observation");

    owner.observe_at(&other_engine, observed_at());

    assert_eq!(owner.readback(), ClosedConnectionsSnapshot::default());
}

#[test]
fn clear_returns_an_empty_readback_and_keeps_the_baseline() {
    let owner = ClosedConnectionsApplication::new();
    owner.observe_at(&observation(1, vec![connection("c-1")]), observed_at());
    owner.observe_at(&observation(2, vec![]), observed_at());

    assert_eq!(owner.clear(), ClosedConnectionsSnapshot::default());
    owner.observe_at(&observation(3, vec![connection("c-1")]), observed_at());
    owner.observe_at(&observation(4, vec![]), observed_at());

    assert_eq!(owner.readback().total_closed, 1);
}
