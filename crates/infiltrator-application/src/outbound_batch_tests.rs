use crate::outbound_application::OutboundApplication;
use async_trait::async_trait;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, EngineId,
};
use lurker_core_contract::outbound::{
    details::{
        OutboundCollectionKind, OutboundDetail, OutboundDetailsSnapshot, OutboundDetailsState,
        OutboundHealthObservation, OutboundObservation, OutboundProbeAvailability,
        OutboundProtocolObservation, OutboundSnapshot, OutboundUdpSupport,
    },
    identity::{OutboundId, SnapshotRevision},
    inventory::{
        OutboundInventorySnapshot, OutboundInventoryState, OutboundObservationSnapshot,
        OutboundProbeSnapshot, OutboundProbeState, OutboundSelectionSet, OutboundSelectionState,
        SelectionAvailability,
    },
    probe::{
        OutboundProbeBatchOutcome, OutboundProbeBatchScope, OutboundProbeConcurrency,
        ProbeOutboundBatchRequest,
    },
};
use lurker_core_ports::error::PortError;
use lurker_core_ports::outbound::{OutboundPort, OutboundProbeRequest};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::time::sleep;

struct BatchPort {
    observation: OutboundObservationSnapshot,
    observation_reads: AtomicUsize,
    calls: Mutex<Vec<(String, SnapshotRevision, String, u64)>>,
    result_states: HashMap<String, OutboundProbeState>,
    active_probes: AtomicUsize,
    peak_probes: AtomicUsize,
}

impl BatchPort {
    fn new(
        observation: OutboundObservationSnapshot,
        result_states: HashMap<String, OutboundProbeState>,
    ) -> Self {
        Self {
            observation,
            observation_reads: AtomicUsize::new(0),
            calls: Mutex::new(Vec::new()),
            result_states,
            active_probes: AtomicUsize::new(0),
            peak_probes: AtomicUsize::new(0),
        }
    }
}

#[async_trait]
impl OutboundPort for BatchPort {
    async fn inventory(&self, _scope: CoreScope) -> Result<OutboundInventorySnapshot, PortError> {
        unreachable!("the use case reads one coherent outbound observation")
    }

    async fn observation(
        &self,
        _scope: CoreScope,
    ) -> Result<OutboundObservationSnapshot, PortError> {
        self.observation_reads.fetch_add(1, Ordering::SeqCst);
        Ok(self.observation.clone())
    }

    async fn details(
        &self,
        _scope: CoreScope,
        _expected_revision: SnapshotRevision,
    ) -> Result<OutboundDetailsSnapshot, PortError> {
        unreachable!("the use case reads one coherent outbound observation")
    }

    async fn select(
        &self,
        _scope: CoreScope,
        _expected_revision: SnapshotRevision,
        _collection_id: OutboundId,
        _outbound_id: OutboundId,
    ) -> Result<OutboundInventorySnapshot, PortError> {
        unreachable!("batch probing does not select outbounds")
    }

    async fn probe(
        &self,
        scope: CoreScope,
        expected_revision: SnapshotRevision,
        outbound_id: OutboundId,
        request: OutboundProbeRequest,
    ) -> Result<OutboundProbeSnapshot, PortError> {
        let active = self.active_probes.fetch_add(1, Ordering::SeqCst) + 1;
        self.peak_probes.fetch_max(active, Ordering::SeqCst);
        self.calls.lock().expect("probe call log").push((
            outbound_id.value().to_owned(),
            expected_revision,
            request.target().as_url().to_owned(),
            request.timeout_ms(),
        ));
        sleep(Duration::from_millis(15)).await;
        self.active_probes.fetch_sub(1, Ordering::SeqCst);
        let state = self
            .result_states
            .get(outbound_id.value())
            .cloned()
            .unwrap_or(OutboundProbeState::Measured { delay_ms: 20 });
        OutboundProbeSnapshot::new(
            scope,
            expected_revision,
            SnapshotRevision::new(3),
            outbound_id,
            state,
        )
        .map_err(|error| PortError::Failed(error.to_string()))
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

fn observation(
    scope: CoreScope,
    probe_availability: &[(String, OutboundProbeAvailability)],
) -> OutboundObservationSnapshot {
    let engine = scope.identity.engine.clone();
    let entries = [
        "selector/main",
        "selector/nested",
        "proxy-a",
        "proxy-b",
        "proxy-c",
        "proxy-d",
        "proxy-e",
    ]
    .into_iter()
    .map(|name| OutboundSnapshot {
        id: id(engine.as_str(), name),
        display_name: None,
    })
    .collect();
    let collections = vec![
        OutboundSelectionSet {
            id: id(engine.as_str(), "selector/main"),
            member_ids: vec![
                id(engine.as_str(), "proxy-a"),
                id(engine.as_str(), "proxy-b"),
                id(engine.as_str(), "selector/nested"),
            ],
            availability: SelectionAvailability::Selectable,
            selection: OutboundSelectionState::Ready {
                selected: id(engine.as_str(), "proxy-a"),
            },
        },
        OutboundSelectionSet {
            id: id(engine.as_str(), "selector/nested"),
            member_ids: vec![
                id(engine.as_str(), "proxy-c"),
                id(engine.as_str(), "proxy-d"),
                id(engine.as_str(), "proxy-e"),
            ],
            availability: SelectionAvailability::Selectable,
            selection: OutboundSelectionState::Ready {
                selected: id(engine.as_str(), "proxy-c"),
            },
        },
    ];
    let inventory = OutboundInventorySnapshot::new(
        scope.clone(),
        SnapshotRevision::new(12),
        OutboundInventoryState::Ready {
            entries,
            collections,
        },
    )
    .expect("valid outbound inventory");
    let details = [
        "selector/main",
        "selector/nested",
        "proxy-a",
        "proxy-b",
        "proxy-c",
        "proxy-d",
        "proxy-e",
    ]
    .into_iter()
    .map(|name| OutboundDetail {
        id: id(engine.as_str(), name),
        protocol: OutboundProtocolObservation::Unknown,
        protocol_display_name: None,
        cipher: OutboundObservation::Unsupported,
        cipher_display_name: None,
        collection_kind: if name.starts_with("selector/") {
            OutboundObservation::Observed(OutboundCollectionKind::ManualSelection)
        } else {
            OutboundObservation::Unsupported
        },
        features: OutboundObservation::Unknown,
        endpoint: OutboundObservation::Unknown,
        udp: OutboundUdpSupport::Unknown,
        health: OutboundHealthObservation::Unknown,
        current_latency_ms: OutboundObservation::Unknown,
        latency_history: OutboundObservation::Unknown,
        rtt: OutboundObservation::Unknown,
        historical_egress: OutboundObservation::Unsupported,
        probe_availability: probe_availability
            .iter()
            .find(|(target, _)| target == name)
            .map_or(OutboundProbeAvailability::Supported, |(_, availability)| {
                *availability
            }),
    })
    .collect();
    let details = OutboundDetailsSnapshot::new(
        scope,
        SnapshotRevision::new(2),
        OutboundDetailsState::Ready { details },
    )
    .expect("valid outbound details");
    OutboundObservationSnapshot::new(inventory, details).expect("coherent observation")
}

fn request(
    target_scope: OutboundProbeBatchScope,
    revision: u64,
    concurrency: u8,
) -> ProbeOutboundBatchRequest {
    ProbeOutboundBatchRequest::new(
        target_scope,
        SnapshotRevision::new(revision),
        "https://health.example.test/check",
        4_000,
        OutboundProbeConcurrency::new(concurrency).expect("valid concurrency"),
    )
    .expect("valid batch request")
}

#[tokio::test]
async fn all_batch_probes_only_concrete_members_once_with_bounded_concurrency() {
    let scope = scope("mihomo", 5);
    let observation = observation(
        scope.clone(),
        &[
            ("proxy-c".to_owned(), OutboundProbeAvailability::Unsupported),
            ("proxy-e".to_owned(), OutboundProbeAvailability::Unknown),
        ],
    );
    let port = Arc::new(BatchPort::new(
        observation,
        HashMap::from([
            (
                "proxy-a".to_owned(),
                OutboundProbeState::Measured { delay_ms: 0 },
            ),
            ("proxy-b".to_owned(), OutboundProbeState::TimedOut),
            (
                "proxy-d".to_owned(),
                OutboundProbeState::Failed {
                    failure: Failure::new(ErrorCode::Network, "probe failed", true),
                },
            ),
        ]),
    ));
    let application = OutboundApplication::new(port.clone());
    let request = request(OutboundProbeBatchScope::All, 12, 2);

    let snapshot = application
        .probe_batch_request(&scope, &request)
        .await
        .expect("all concrete outbounds are probed");

    assert_eq!(port.observation_reads.load(Ordering::SeqCst), 1);
    assert_eq!(
        snapshot
            .items()
            .iter()
            .map(|item| item.outbound_id.value())
            .collect::<Vec<_>>(),
        ["proxy-a", "proxy-b", "proxy-c", "proxy-d", "proxy-e"]
    );
    assert_eq!(
        snapshot
            .items()
            .iter()
            .map(|item| &item.outcome)
            .collect::<Vec<_>>(),
        [
            &OutboundProbeBatchOutcome::Measured { delay_ms: 0 },
            &OutboundProbeBatchOutcome::TimedOut,
            &OutboundProbeBatchOutcome::Unsupported,
            &OutboundProbeBatchOutcome::Failed {
                failure: Failure::new(ErrorCode::Network, "probe failed", true),
            },
            &OutboundProbeBatchOutcome::Failed {
                failure: Failure::new(
                    ErrorCode::NotReady,
                    "outbound probe availability has not been observed",
                    true,
                ),
            },
        ]
    );
    let calls = port.calls.lock().expect("probe call log");
    assert_eq!(calls.len(), 3);
    assert!(calls.iter().all(|call| {
        call.1 == SnapshotRevision::new(12)
            && call.2 == "https://health.example.test/check"
            && call.3 == 4_000
    }));
    assert!(!calls.iter().any(|call| call.0.starts_with("selector/")));
    assert!(
        !calls
            .iter()
            .any(|call| call.0 == "proxy-c" || call.0 == "proxy-e")
    );
    assert_eq!(port.peak_probes.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn collection_batch_expands_direct_concrete_members_and_rejects_stale_or_wrong_scope_before_probe()
 {
    let active_scope = scope("mihomo", 5);
    let port = Arc::new(BatchPort::new(
        observation(active_scope.clone(), &[]),
        HashMap::new(),
    ));
    let application = OutboundApplication::new(port.clone());
    let collection_request = request(
        OutboundProbeBatchScope::Collection(id("mihomo", "selector/main")),
        12,
        1,
    );

    let snapshot = application
        .probe_batch_request(&active_scope, &collection_request)
        .await
        .expect("collection members are probed");
    assert_eq!(
        snapshot
            .items()
            .iter()
            .map(|item| item.outbound_id.value())
            .collect::<Vec<_>>(),
        ["proxy-a", "proxy-b"]
    );
    assert_eq!(port.calls.lock().expect("probe call log").len(), 2);

    let stale = request(OutboundProbeBatchScope::All, 99, 2);
    let failure = application
        .probe_batch_request(&active_scope, &stale)
        .await
        .expect_err("stale inventory is rejected before probes");
    assert_eq!(failure.code, ErrorCode::InvalidState);
    assert_eq!(port.calls.lock().expect("probe call log").len(), 2);

    let wrong_scope = request(
        OutboundProbeBatchScope::One(id("sing-box", "proxy-a")),
        12,
        1,
    );
    let failure = application
        .probe_batch_request(&active_scope, &wrong_scope)
        .await
        .expect_err("wrong engine is rejected before the observation read");
    assert_eq!(failure.code, ErrorCode::InvalidState);
    assert_eq!(port.observation_reads.load(Ordering::SeqCst), 2);
    assert_eq!(port.calls.lock().expect("probe call log").len(), 2);
}

#[tokio::test]
async fn selected_batch_preserves_order_and_rejects_unknown_or_collection_targets_before_probe() {
    let scope = scope("mihomo", 5);
    let port = Arc::new(BatchPort::new(
        observation(scope.clone(), &[]),
        HashMap::new(),
    ));
    let application = OutboundApplication::new(port.clone());
    let selected = request(
        OutboundProbeBatchScope::Selected(vec![id("mihomo", "proxy-d"), id("mihomo", "proxy-a")]),
        12,
        1,
    );

    let snapshot = application
        .probe_batch_request(&scope, &selected)
        .await
        .expect("selected concrete targets are probed");
    assert_eq!(
        snapshot
            .items()
            .iter()
            .map(|item| item.outbound_id.value())
            .collect::<Vec<_>>(),
        ["proxy-d", "proxy-a"]
    );
    assert_eq!(port.calls.lock().expect("probe call log").len(), 2);

    let unknown = request(
        OutboundProbeBatchScope::Selected(vec![id("mihomo", "proxy-a"), id("mihomo", "missing")]),
        12,
        1,
    );
    let failure = application
        .probe_batch_request(&scope, &unknown)
        .await
        .expect_err("unknown member rejects the complete batch");
    assert_eq!(failure.code, ErrorCode::InvalidInput);
    assert_eq!(port.calls.lock().expect("probe call log").len(), 2);

    let collection = request(
        OutboundProbeBatchScope::Selected(vec![id("mihomo", "selector/main")]),
        12,
        1,
    );
    let failure = application
        .probe_batch_request(&scope, &collection)
        .await
        .expect_err("a selected collection is rejected as a target");
    assert_eq!(failure.code, ErrorCode::InvalidInput);
    assert_eq!(port.calls.lock().expect("probe call log").len(), 2);
}
