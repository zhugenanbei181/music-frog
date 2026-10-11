use super::*;
use async_trait::async_trait;
use infiltrator_contract::core_identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, EngineId,
};
use infiltrator_contract::outbound::details::{
    OutboundCollectionKind, OutboundDetail, OutboundDetailsSnapshot, OutboundDetailsState,
    OutboundHealthObservation, OutboundObservation, OutboundProbeAvailability,
    OutboundProtocolObservation, OutboundSnapshot, OutboundUdpSupport,
};
use infiltrator_contract::outbound::identity::{OutboundId, SnapshotRevision};
use infiltrator_contract::outbound::inventory::{
    OutboundInventorySnapshot, OutboundInventoryState, OutboundObservationSnapshot,
    OutboundProbeSnapshot, OutboundProbeState, OutboundSelectionSet, OutboundSelectionState,
    SelectionAvailability,
};
use infiltrator_contract::outbound::probe::OutboundProbeBatchScope;
use infiltrator_contract::runtime_settings::RuntimeProbeSettings;
use infiltrator_contract::speedtest::{
    SpeedtestResultState, SpeedtestRounds, SpeedtestRunState, SpeedtestTarget,
};
use infiltrator_domain::settings::AppSettings;
use infiltrator_ports::error::PortError;
use infiltrator_ports::outbound::{OutboundPort, OutboundProbeRequest};
use infiltrator_ports::settings_store::SettingsStore;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;

struct ProbeSettingsStore {
    settings: AppSettings,
}

#[async_trait]
impl SettingsStore for ProbeSettingsStore {
    async fn load(&self) -> Result<AppSettings, PortError> {
        Ok(self.settings.clone())
    }

    async fn load_hydrated(&self) -> Result<AppSettings, PortError> {
        self.load().await
    }

    async fn save(&self, _settings: &AppSettings) -> Result<(), PortError> {
        Ok(())
    }
}

struct FailedSettingsStore;

#[async_trait]
impl SettingsStore for FailedSettingsStore {
    async fn load(&self) -> Result<AppSettings, PortError> {
        Err(PortError::Io("probe settings unavailable".to_owned()))
    }

    async fn load_hydrated(&self) -> Result<AppSettings, PortError> {
        self.load().await
    }

    async fn save(&self, _settings: &AppSettings) -> Result<(), PortError> {
        Err(PortError::Io("probe settings unavailable".to_owned()))
    }
}

#[derive(Default)]
pub(crate) struct ProbeGate {
    pub(crate) started: Notify,
    release: Notify,
}

pub(crate) struct FakeOutboundPort {
    observation: OutboundObservationSnapshot,
    outcomes: Mutex<VecDeque<OutboundProbeState>>,
    calls: Mutex<Vec<OutboundId>>,
    gate: Option<Arc<ProbeGate>>,
}

impl FakeOutboundPort {
    fn new(
        observation: OutboundObservationSnapshot,
        outcomes: impl IntoIterator<Item = OutboundProbeState>,
    ) -> Self {
        Self {
            observation,
            outcomes: Mutex::new(outcomes.into_iter().collect()),
            calls: Mutex::new(Vec::new()),
            gate: None,
        }
    }

    pub(crate) fn gated(observation: OutboundObservationSnapshot, gate: Arc<ProbeGate>) -> Self {
        Self {
            observation,
            outcomes: Mutex::new(VecDeque::from([OutboundProbeState::Measured {
                delay_ms: 24,
            }])),
            calls: Mutex::new(Vec::new()),
            gate: Some(gate),
        }
    }

    fn calls(&self) -> Vec<OutboundId> {
        self.calls.lock().expect("probe call log").clone()
    }
}

#[async_trait]
impl OutboundPort for FakeOutboundPort {
    async fn inventory(&self, _scope: CoreScope) -> Result<OutboundInventorySnapshot, PortError> {
        Ok(self.observation.inventory().clone())
    }

    async fn observation(
        &self,
        _scope: CoreScope,
    ) -> Result<OutboundObservationSnapshot, PortError> {
        Ok(self.observation.clone())
    }

    async fn details(
        &self,
        _scope: CoreScope,
        _expected_revision: SnapshotRevision,
    ) -> Result<OutboundDetailsSnapshot, PortError> {
        Ok(self.observation.details().clone())
    }

    async fn select(
        &self,
        _scope: CoreScope,
        _expected_revision: SnapshotRevision,
        _collection_id: OutboundId,
        _outbound_id: OutboundId,
    ) -> Result<OutboundInventorySnapshot, PortError> {
        unreachable!("speedtest does not select an outbound")
    }

    async fn probe(
        &self,
        scope: CoreScope,
        expected_revision: SnapshotRevision,
        outbound_id: OutboundId,
        _request: OutboundProbeRequest,
    ) -> Result<OutboundProbeSnapshot, PortError> {
        self.calls
            .lock()
            .expect("probe call log")
            .push(outbound_id.clone());
        if let Some(gate) = &self.gate {
            gate.started.notify_one();
            gate.release.notified().await;
        }
        let outcome = self
            .outcomes
            .lock()
            .expect("probe outcomes")
            .pop_front()
            .unwrap_or(OutboundProbeState::Measured { delay_ms: 25 });
        OutboundProbeSnapshot::new(
            scope,
            expected_revision,
            SnapshotRevision::new(3),
            outbound_id,
            outcome,
        )
        .map_err(|error| PortError::Failed(error.to_string()))
    }
}

pub(crate) fn scope(engine: &str, generation: u64) -> CoreScope {
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

pub(crate) fn id(engine: &str, value: &str) -> OutboundId {
    OutboundId::new(EngineId::new(engine).expect("engine ID"), value).expect("outbound ID")
}

pub(crate) fn observation(scope: CoreScope, revision: u64) -> OutboundObservationSnapshot {
    let engine = scope.identity.engine.clone();
    let group = id(engine.as_str(), "group/main");
    let first = id(engine.as_str(), "outbound-a");
    let second = id(engine.as_str(), "outbound-b");
    let inventory = OutboundInventorySnapshot::new(
        scope.clone(),
        SnapshotRevision::new(revision),
        OutboundInventoryState::Ready {
            entries: vec![
                OutboundSnapshot {
                    id: group.clone(),
                    display_name: Some("Main group".to_owned()),
                },
                OutboundSnapshot {
                    id: first.clone(),
                    display_name: Some("Outbound A".to_owned()),
                },
                OutboundSnapshot {
                    id: second.clone(),
                    display_name: Some("Outbound B".to_owned()),
                },
            ],
            collections: vec![OutboundSelectionSet {
                id: group.clone(),
                member_ids: vec![first.clone(), second.clone()],
                availability: SelectionAvailability::Selectable,
                selection: OutboundSelectionState::Ready {
                    selected: first.clone(),
                },
            }],
        },
    )
    .expect("valid inventory");
    let details = OutboundDetailsSnapshot::new(
        scope,
        SnapshotRevision::new(2),
        OutboundDetailsState::Ready {
            details: [group, first, second]
                .into_iter()
                .map(|outbound_id| {
                    let is_group = outbound_id.value() == "group/main";
                    OutboundDetail {
                        id: outbound_id,
                        protocol: OutboundProtocolObservation::Unknown,
                        protocol_display_name: None,
                        cipher: OutboundObservation::Unsupported,
                        cipher_display_name: None,
                        collection_kind: if is_group {
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
                        probe_availability: OutboundProbeAvailability::Supported,
                    }
                })
                .collect(),
        },
    )
    .expect("valid details");
    OutboundObservationSnapshot::new(inventory, details).expect("coherent observation")
}

pub(crate) fn app_settings() -> SettingsApplication {
    let mut settings = AppSettings::default();
    settings.runtime_panel.probe_settings =
        RuntimeProbeSettings::new("https://probe.example.test/health", 1500)
            .expect("valid accepted probe settings");
    SettingsApplication::new(Arc::new(ProbeSettingsStore { settings }))
}

#[tokio::test]
async fn unavailable_accepted_probe_settings_fail_closed_without_default_fallback() {
    let owner_scope = scope("mihomo", 4);
    let port = Arc::new(FakeOutboundPort::new(
        observation(owner_scope.clone(), 11),
        [OutboundProbeState::Measured { delay_ms: 20 }],
    ));
    let application =
        SpeedtestApplication::new(SettingsApplication::new(Arc::new(FailedSettingsStore)));
    application
        .install_outbound_application(OutboundApplication::new(port.clone()))
        .expect("install one outbound owner");
    let request = target(
        owner_scope,
        11,
        OutboundProbeBatchScope::One(id("mihomo", "outbound-a")),
        3,
    );

    assert!(application.prepare(request).await.is_err());
    assert!(matches!(
        application.snapshot().state,
        SpeedtestRunState::Idle
    ));
    assert!(port.calls().is_empty());
}

pub(crate) fn target(
    scope: CoreScope,
    revision: u64,
    outbound_scope: OutboundProbeBatchScope,
    rounds: u8,
) -> SpeedtestTarget {
    SpeedtestTarget::new(
        scope,
        SnapshotRevision::new(revision),
        outbound_scope,
        SpeedtestRounds::new(rounds).expect("bounded rounds"),
    )
    .expect("valid typed target")
}

#[tokio::test]
async fn one_observed_collection_runs_multiple_typed_probe_rounds_per_member() {
    let owner_scope = scope("mihomo", 4);
    let port = Arc::new(FakeOutboundPort::new(
        observation(owner_scope.clone(), 11),
        [
            OutboundProbeState::Measured { delay_ms: 20 },
            OutboundProbeState::TimedOut,
            OutboundProbeState::Measured { delay_ms: 28 },
            OutboundProbeState::Measured { delay_ms: 30 },
            OutboundProbeState::Measured { delay_ms: 34 },
            OutboundProbeState::Measured { delay_ms: 32 },
        ],
    ));
    let application = SpeedtestApplication::new(app_settings());
    application
        .install_outbound_application(OutboundApplication::new(port.clone()))
        .expect("install one outbound owner");
    application.set_concurrency(SpeedtestConcurrency::new(1).unwrap());
    let request = target(
        owner_scope.clone(),
        11,
        OutboundProbeBatchScope::Collection(id("mihomo", "group/main")),
        3,
    );

    let prepared = application
        .prepare(request.clone())
        .await
        .expect("preflight accepted");
    let snapshot = run_prepared(&application, prepared).await;

    assert_eq!(port.calls().len(), 6);
    assert_eq!(snapshot.results.len(), 2);
    assert_eq!(snapshot.state.target(), Some(&request));
    assert_eq!(snapshot.results[0].samples.len(), 3);
    assert!(matches!(
        &snapshot.results[0].state,
        SpeedtestResultState::Completed
    ));
    let calculation = snapshot.results[0].calculation().expect("sample metrics");
    assert_eq!(calculation.sample_count, 3);
    assert_eq!(calculation.successful_probes, 2);
    assert_eq!(calculation.lost_probes, 1);
    assert_eq!(snapshot.results[1].outbound_id.value(), "outbound-b");
}

#[tokio::test]
async fn foreign_core_and_stale_inventory_targets_fail_before_any_probe() {
    let owner_scope = scope("mihomo", 4);
    let port = Arc::new(FakeOutboundPort::new(
        observation(owner_scope.clone(), 11),
        [OutboundProbeState::Measured { delay_ms: 20 }],
    ));
    let application = SpeedtestApplication::new(app_settings());
    application
        .install_outbound_application(OutboundApplication::new(port.clone()))
        .expect("install one outbound owner");

    let stale = target(
        owner_scope.clone(),
        12,
        OutboundProbeBatchScope::One(id("mihomo", "outbound-a")),
        3,
    );
    assert!(application.prepare(stale).await.is_err());
    assert!(port.calls().is_empty());

    let non_member = target(
        owner_scope.clone(),
        11,
        OutboundProbeBatchScope::One(id("mihomo", "not-in-inventory")),
        3,
    );
    assert!(application.prepare(non_member).await.is_err());
    assert!(port.calls().is_empty());

    let foreign_value = serde_json::to_value(target(
        owner_scope,
        11,
        OutboundProbeBatchScope::One(id("mihomo", "outbound-a")),
        3,
    ))
    .expect("encode target");
    let mut foreign_value = foreign_value;
    foreign_value["core_scope"]["identity"]["engine"] = serde_json::json!("sing-box");
    let foreign: SpeedtestTarget = serde_json::from_value(foreign_value).expect("typed wire value");
    assert!(application.prepare(foreign).await.is_err());
    assert!(port.calls().is_empty());
}

#[tokio::test]
async fn cancellation_aborts_the_probe_future_and_old_run_messages_cannot_replace_a_new_run() {
    let owner_scope = scope("mihomo", 4);
    let gate = Arc::new(ProbeGate::default());
    let port = Arc::new(FakeOutboundPort::gated(
        observation(owner_scope.clone(), 11),
        gate.clone(),
    ));
    let application = SpeedtestApplication::new(app_settings());
    application
        .install_outbound_application(OutboundApplication::new(port))
        .expect("install one outbound owner");
    let request = target(
        owner_scope.clone(),
        11,
        OutboundProbeBatchScope::One(id("mihomo", "outbound-a")),
        4,
    );
    let prepared = application
        .prepare(request)
        .await
        .expect("preflight accepted");
    let admission = application.admit(prepared).expect("run accepted");
    let run_id = admission.receipt.run_id;
    let run_scope = admission.prepared.target.core_scope().clone();
    let super::SpeedtestRunAdmission {
        receipt,
        registration,
        prepared,
        concurrency,
    } = admission;
    let start = gate.started.notified();
    tokio::pin!(start);
    let run_application = application.clone();
    let run = tokio::spawn(async move {
        let outcome = match futures_util::future::Abortable::new(
            run_application.execute_prepared(receipt.run_id, prepared, concurrency),
            registration,
        )
        .await
        {
            Ok(()) => SpeedtestTaskOutcome::Completed,
            Err(_) => SpeedtestTaskOutcome::Cancelled,
        };
        (receipt.run_id, outcome)
    });
    start.await;
    assert!(matches!(
        application.snapshot().state,
        SpeedtestRunState::Probing { run_id: active, .. } if active == run_id
    ));
    assert!(matches!(
        application.cancel_run(run_id),
        SpeedtestCancelOutcome::Requested { run_id: cancelled } if cancelled == run_id
    ));
    let (completed_run, outcome) = run.await.expect("probe future join");
    assert_eq!(completed_run, run_id);
    assert_eq!(outcome, SpeedtestTaskOutcome::Cancelled);
    assert_eq!(
        application.finish(run_id, &run_scope, Some(&run_scope), outcome),
        SpeedtestCompletionAdmission::Applied
    );
    assert!(matches!(
        application.snapshot().state,
        SpeedtestRunState::Cancelled { .. }
    ));

    let next_scope = scope("mihomo", 4);
    let next_prepared = application
        .prepare(target(
            next_scope.clone(),
            11,
            OutboundProbeBatchScope::One(id("mihomo", "outbound-a")),
            1,
        ))
        .await
        .expect("next preflight accepted");
    let next = application.admit(next_prepared).expect("next run accepted");
    let next_run_id = next.receipt.run_id;
    assert!(next_run_id > run_id);
    assert!(matches!(
        application.finish(run_id, &run_scope, Some(&next_scope), SpeedtestTaskOutcome::Completed),
        SpeedtestCompletionAdmission::IgnoredStaleRun { current: Some(active), .. }
            if active == next_run_id
    ));
    assert!(matches!(
        application.snapshot().state,
        SpeedtestRunState::Probing { run_id: active, .. } if active == next_run_id
    ));
}

async fn run_prepared(
    application: &SpeedtestApplication,
    prepared: super::PreparedSpeedtestRun,
) -> SpeedtestSnapshot {
    let admission = application.admit(prepared).expect("run accepted");
    let super::SpeedtestRunAdmission {
        receipt,
        registration,
        prepared,
        concurrency,
    } = admission;
    let scope = prepared.target.core_scope().clone();
    let outcome = match futures_util::future::Abortable::new(
        application.execute_prepared(receipt.run_id, prepared, concurrency),
        registration,
    )
    .await
    {
        Ok(()) => SpeedtestTaskOutcome::Completed,
        Err(_) => SpeedtestTaskOutcome::Cancelled,
    };
    application.finish(receipt.run_id, &scope, Some(&scope), outcome);
    application.snapshot()
}
