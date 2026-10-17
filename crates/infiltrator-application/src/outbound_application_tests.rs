use crate::outbound_application::OutboundApplication;
use async_trait::async_trait;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, EngineId,
};
use lurker_core_contract::outbound::{
    details::{
        OutboundDetail, OutboundDetailsSnapshot, OutboundDetailsState, OutboundHealthObservation,
        OutboundObservation, OutboundProbeAvailability, OutboundProtocolObservation,
        OutboundSnapshot, OutboundUdpSupport,
    },
    identity::{OutboundId, SnapshotRevision},
    inventory::{
        OutboundInventorySnapshot, OutboundInventoryState, OutboundProbeSnapshot,
        OutboundSelectionSet, OutboundSelectionState, SelectionAvailability,
    },
};
use lurker_core_ports::error::PortError;
use lurker_core_ports::outbound::{OutboundPort, OutboundProbeRequest};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Calls {
    selection: Option<(CoreScope, SnapshotRevision, OutboundId, OutboundId)>,
    probe: Option<(CoreScope, SnapshotRevision, OutboundId)>,
}

struct StubOutboundPort {
    inventory: OutboundInventorySnapshot,
    selection: Option<OutboundInventorySnapshot>,
    probe: Option<OutboundProbeSnapshot>,
    calls: Mutex<Calls>,
}

impl StubOutboundPort {
    fn new(
        inventory: OutboundInventorySnapshot,
        selection: Option<OutboundInventorySnapshot>,
        probe: Option<OutboundProbeSnapshot>,
    ) -> Self {
        Self {
            inventory,
            selection,
            probe,
            calls: Mutex::new(Calls::default()),
        }
    }
}

#[async_trait]
impl OutboundPort for StubOutboundPort {
    async fn inventory(&self, _scope: CoreScope) -> Result<OutboundInventorySnapshot, PortError> {
        Ok(self.inventory.clone())
    }

    async fn details(
        &self,
        scope: CoreScope,
        _expected_revision: SnapshotRevision,
    ) -> Result<OutboundDetailsSnapshot, PortError> {
        let OutboundInventoryState::Ready { entries, .. } = self.inventory.inventory() else {
            return Err(PortError::Rejected(Failure::unsupported(
                "test inventory has no details",
            )));
        };
        let details = entries
            .iter()
            .map(|entry| OutboundDetail {
                id: entry.id.clone(),
                protocol: OutboundProtocolObservation::Unknown,
                protocol_display_name: None,
                cipher: OutboundObservation::Unsupported,
                cipher_display_name: None,
                collection_kind: OutboundObservation::Unsupported,
                features: OutboundObservation::Unknown,
                endpoint: OutboundObservation::Unknown,
                udp: OutboundUdpSupport::Unknown,
                health: OutboundHealthObservation::Unknown,
                current_latency_ms: OutboundObservation::Unknown,
                latency_history: OutboundObservation::Unknown,
                rtt: OutboundObservation::Unknown,
                historical_egress: OutboundObservation::Unsupported,
                probe_availability: OutboundProbeAvailability::Supported,
            })
            .collect();
        OutboundDetailsSnapshot::new(
            scope,
            SnapshotRevision::new(1),
            OutboundDetailsState::Ready { details },
        )
        .map_err(|error| PortError::Failed(error.to_string()))
    }

    async fn select(
        &self,
        scope: CoreScope,
        expected_revision: SnapshotRevision,
        collection_id: OutboundId,
        outbound_id: OutboundId,
    ) -> Result<OutboundInventorySnapshot, PortError> {
        self.calls.lock().expect("calls lock").selection =
            Some((scope, expected_revision, collection_id, outbound_id));
        self.selection
            .clone()
            .ok_or_else(|| PortError::Failed("selection response missing".to_owned()))
    }

    async fn probe(
        &self,
        scope: CoreScope,
        expected_revision: SnapshotRevision,
        outbound_id: OutboundId,
        _request: OutboundProbeRequest,
    ) -> Result<OutboundProbeSnapshot, PortError> {
        self.calls.lock().expect("calls lock").probe =
            Some((scope, expected_revision, outbound_id));
        self.probe
            .clone()
            .ok_or_else(|| PortError::Failed("probe response missing".to_owned()))
    }
}

fn scope(engine: &str, generation: u64) -> CoreScope {
    CoreScope {
        identity: CoreIdentity::new(
            EngineId::new(engine).expect("engine ID"),
            CoreInstanceId::new("primary").expect("instance ID"),
            None,
        ),
        generation: CoreGeneration::new(generation),
        session_token: None,
    }
}

fn id(engine: &str, value: &str) -> OutboundId {
    OutboundId::new(EngineId::new(engine).expect("engine ID"), value).expect("outbound ID")
}

fn inventory(scope: CoreScope, revision: u64, selected: &str) -> OutboundInventorySnapshot {
    let engine = scope.identity.engine.clone();
    OutboundInventorySnapshot::new(
        scope,
        SnapshotRevision::new(revision),
        OutboundInventoryState::Ready {
            entries: vec![
                OutboundSnapshot {
                    id: id(engine.as_str(), "proxy-a"),
                    display_name: Some("Proxy A".to_owned()),
                },
                OutboundSnapshot {
                    id: id(engine.as_str(), "proxy-b"),
                    display_name: None,
                },
            ],
            collections: vec![OutboundSelectionSet {
                id: id(engine.as_str(), "selector/main"),
                member_ids: vec![
                    id(engine.as_str(), "proxy-a"),
                    id(engine.as_str(), "proxy-b"),
                ],
                availability: SelectionAvailability::Selectable,
                selection: OutboundSelectionState::Ready {
                    selected: id(engine.as_str(), selected),
                },
            }],
        },
    )
    .expect("valid outbound inventory")
}

#[tokio::test]
async fn inventory_rejects_a_snapshot_from_another_core_activation() {
    let requested_scope = scope("sing-box", 8);
    let adapter = Arc::new(StubOutboundPort::new(
        inventory(scope("sing-box", 7), 20, "proxy-a"),
        None,
        None,
    ));
    let application = OutboundApplication::new(adapter);

    let failure = application
        .inventory(requested_scope)
        .await
        .expect_err("stale activation is rejected");

    assert_eq!(failure.code, ErrorCode::InvalidState);
    assert!(failure.message.contains("different core scope"));
}

#[tokio::test]
async fn adapter_failures_keep_their_typed_failure() {
    struct FailedPort;

    #[async_trait]
    impl OutboundPort for FailedPort {
        async fn inventory(
            &self,
            _scope: CoreScope,
        ) -> Result<OutboundInventorySnapshot, PortError> {
            Err(PortError::Rejected(Failure::new(
                ErrorCode::Network,
                "outbound endpoint unavailable",
                true,
            )))
        }

        async fn select(
            &self,
            _scope: CoreScope,
            _expected_revision: SnapshotRevision,
            _collection_id: OutboundId,
            _outbound_id: OutboundId,
        ) -> Result<OutboundInventorySnapshot, PortError> {
            unreachable!("not called")
        }

        async fn probe(
            &self,
            _scope: CoreScope,
            _expected_revision: SnapshotRevision,
            _outbound_id: OutboundId,
            _request: OutboundProbeRequest,
        ) -> Result<OutboundProbeSnapshot, PortError> {
            unreachable!("not called")
        }
    }

    let application = OutboundApplication::new(Arc::new(FailedPort));
    let failure = application
        .inventory(scope("mihomo", 1))
        .await
        .expect_err("adapter failure is returned");
    assert_eq!(failure.code, ErrorCode::Network);
    assert_eq!(failure.message, "outbound endpoint unavailable");
}
