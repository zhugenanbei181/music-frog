//! Headless tests for the typed `ClearClosedConnections` owner command.

use super::*;
use crate::closed_connections_application::ClosedConnectionsApplication;
use infiltrator_domain::runtime::{Connection, ConnectionMetadata, ConnectionSnapshot};
use infiltrator_ports::connection::ConnectionObservationSnapshot;
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

fn observation(connections: Vec<Connection>) -> ConnectionObservationSnapshot {
    ConnectionObservationSnapshot::new(
        scope(),
        SnapshotRevision::new(1),
        ConnectionSnapshot {
            connections,
            ..Default::default()
        },
    )
    .expect("valid connection observation")
}

#[tokio::test]
async fn clearing_closed_connections_resets_the_shared_history_owner() {
    let owner = ClosedConnectionsApplication::new();
    owner.observe(&observation(vec![Connection {
        id: "c-1".to_owned(),
        metadata: ConnectionMetadata {
            host: "example.com".to_owned(),
            ..Default::default()
        },
        start: "2026-11-12T05:00:00+00:00".to_owned(),
        ..Default::default()
    }]));
    owner.observe(&observation(Vec::new()));
    assert_eq!(owner.readback().total_closed, 1);
    let commands = CommandApplication::new().with_closed_connections(owner.clone());

    commands
        .execute(CommandIntent::ClearClosedConnections)
        .await
        .expect("the shared closed-history owner answers the clear command");

    assert_eq!(owner.readback(), Default::default());
}

#[tokio::test]
async fn clearing_closed_connections_without_an_owner_refuses_instead_of_faking_success() {
    let commands = CommandApplication::new();
    let failure = commands
        .execute(CommandIntent::ClearClosedConnections)
        .await
        .expect_err("no composed closed-history owner");
    assert!(
        failure.message.contains("closed-history owner"),
        "{failure:?}"
    );
}
