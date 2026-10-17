//! Neutral in-memory fakes so a third party can run the kit without an engine.
//!
//! The fakes implement only the neutral ports. They contain no engine-specific
//! behavior: the valid document, collection and members all come from the
//! caller's [`AdapterConformanceCase`].

use super::case::{AdapterConformanceCase, DetailsBehavior, ProbeBehavior, RoutingBehavior};
use crate::switch::memory_journal::MemoryEngineSwitchJournal;
use async_trait::async_trait;
use lurker_core_contract::capability::Capability;
use lurker_core_contract::core_process::CoreProcessStartContext;
use lurker_core_contract::engine_switch::{
    EngineSwitchRecovery, EngineSwitchRevision, EngineSwitchSnapshot,
};
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::{
    CoreGeneration, CoreIdentity, CoreScope, CoreVersion, EngineId,
};
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_contract::outbound::details::{
    OutboundDetail, OutboundDetailsSnapshot, OutboundDetailsState, OutboundHealthObservation,
    OutboundObservation, OutboundProbeAvailability, OutboundProtocolObservation, OutboundSnapshot,
    OutboundUdpSupport,
};
use lurker_core_contract::outbound::identity::{OutboundId, SnapshotRevision};
use lurker_core_contract::outbound::inventory::{
    OutboundInventorySnapshot, OutboundInventoryState, OutboundProbeSnapshot, OutboundProbeState,
    OutboundSelectionSet, OutboundSelectionState, SelectionAvailability,
};
use lurker_core_contract::profile_document::DocumentValidation;
use lurker_core_contract::routing_intent::RoutingIntent;
use lurker_core_ports::core_process::{CoreProcess, CoreReadiness};
use lurker_core_ports::engine_switch::{
    EngineOwnerLease, EngineOwnerStoppedSwitchLease, EngineOwnerSwitchLease, EngineSwitchJournal,
};
use lurker_core_ports::error::PortError;
use lurker_core_ports::outbound::{OutboundPort, OutboundProbeRequest};
use lurker_core_ports::profile_document_codec::{
    ProfileDocumentCodecPort, ProfileDocumentCodecScope,
};
use lurker_core_ports::routing_intent::RoutingIntentPort;
use std::sync::{Arc, Mutex, MutexGuard};

/// Observable lifecycle phase of [`FakeProcess`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FakeProcessPhase {
    Stopped,
    Running,
}

/// Process adapter whose only facts are start/stop and the exact terminal.
pub struct FakeProcess {
    identity: CoreIdentity,
    phase: Arc<Mutex<FakeProcessPhase>>,
}

impl FakeProcess {
    pub fn new(identity: CoreIdentity) -> Self {
        Self {
            identity,
            phase: Arc::new(Mutex::new(FakeProcessPhase::Stopped)),
        }
    }

    /// A readiness adapter that follows this process phase and reports
    /// `version` while running.
    pub fn readiness(&self, version: CoreVersion) -> FakeReadiness {
        FakeReadiness {
            phase: Arc::clone(&self.phase),
            version,
        }
    }

    pub fn phase(&self) -> FakeProcessPhase {
        *self.phase.lock().expect("fake process phase lock")
    }

    fn phase_mut(&self) -> MutexGuard<'_, FakeProcessPhase> {
        self.phase.lock().expect("fake process phase lock")
    }
}

#[async_trait]
impl CoreProcess for FakeProcess {
    fn core_identity(&self) -> Option<CoreIdentity> {
        Some(self.identity.clone())
    }

    async fn start(&self, _context: CoreProcessStartContext) -> Result<(), PortError> {
        let mut phase = self.phase_mut();
        match *phase {
            FakeProcessPhase::Stopped => {
                *phase = FakeProcessPhase::Running;
                Ok(())
            }
            FakeProcessPhase::Running => Err(PortError::Rejected(Failure::new(
                ErrorCode::InvalidState,
                "fake process is already running",
                false,
            ))),
        }
    }

    async fn stop(&self) -> Result<(), PortError> {
        *self.phase_mut() = FakeProcessPhase::Stopped;
        Ok(())
    }

    async fn status(&self) -> Result<CoreLifecycle, PortError> {
        match self.phase() {
            FakeProcessPhase::Stopped => Ok(CoreLifecycle::Stopped),
            FakeProcessPhase::Running => Ok(CoreLifecycle::Running),
        }
    }

    fn controller_endpoint(&self) -> Option<String> {
        Some("http://127.0.0.1:0".to_owned())
    }

    async fn pid(&self) -> Option<u32> {
        match self.phase() {
            FakeProcessPhase::Stopped => None,
            FakeProcessPhase::Running => Some(1),
        }
    }
}

/// Readiness adapter bound to one [`FakeProcess`].
pub struct FakeReadiness {
    phase: Arc<Mutex<FakeProcessPhase>>,
    version: CoreVersion,
}

#[async_trait]
impl CoreReadiness for FakeReadiness {
    async fn probe(&self) -> Result<String, PortError> {
        match *self.phase.lock().expect("fake process phase lock") {
            FakeProcessPhase::Stopped => {
                Err(PortError::Network("fake process is stopped".to_owned()))
            }
            FakeProcessPhase::Running => Ok("fake-ready".to_owned()),
        }
    }

    async fn active_core_version(&self) -> Result<Option<CoreVersion>, PortError> {
        match *self.phase.lock().expect("fake process phase lock") {
            FakeProcessPhase::Stopped => Ok(None),
            FakeProcessPhase::Running => Ok(Some(self.version.clone())),
        }
    }
}

/// Pure document codec for one case.
pub struct FakeCodec {
    scope: ProfileDocumentCodecScope,
    valid_document: Vec<u8>,
    native_invalid_document: Vec<u8>,
}

impl FakeCodec {
    pub fn new(
        scope: ProfileDocumentCodecScope,
        valid_document: &[u8],
        native_invalid_document: &[u8],
    ) -> Self {
        Self {
            scope,
            valid_document: valid_document.to_vec(),
            native_invalid_document: native_invalid_document.to_vec(),
        }
    }

    pub fn for_case(case: &AdapterConformanceCase) -> Self {
        Self::new(
            ProfileDocumentCodecScope {
                engine_id: case.engine.clone(),
                format_id: case.profile_format.clone(),
                schema_version: case.profile_schema,
            },
            &case.valid_document,
            &case.native_invalid_document,
        )
    }
}

impl ProfileDocumentCodecPort for FakeCodec {
    fn scope(&self) -> ProfileDocumentCodecScope {
        self.scope.clone()
    }

    fn inspect(&self, raw_bytes: &[u8]) -> Result<DocumentValidation, PortError> {
        if raw_bytes == self.valid_document || raw_bytes == self.native_invalid_document {
            Ok(DocumentValidation::Valid)
        } else {
            Ok(DocumentValidation::Invalid {
                diagnostics: Vec::new(),
            })
        }
    }
}

/// In-memory outbound inventory and selector for one case and scope.
pub struct FakeOutbound {
    expected_scope: CoreScope,
    collection_id: OutboundId,
    member_ids: Vec<OutboundId>,
    detail_ids: Vec<OutboundId>,
    selected: Mutex<OutboundId>,
    revision: Mutex<u64>,
    probe_outbound_id: OutboundId,
    details: DetailsBehavior,
    probe: ProbeBehavior,
}

impl FakeOutbound {
    pub fn new(scope: CoreScope, case: &AdapterConformanceCase) -> Self {
        let member_ids = vec![
            case.initial_member_id.clone(),
            case.alternate_member_id.clone(),
        ];
        let mut detail_ids = member_ids.clone();
        if !detail_ids.contains(&case.probe_outbound_id) {
            detail_ids.push(case.probe_outbound_id.clone());
        }
        Self {
            expected_scope: scope,
            collection_id: case.collection_id.clone(),
            member_ids,
            detail_ids,
            selected: Mutex::new(case.initial_member_id.clone()),
            revision: Mutex::new(1),
            probe_outbound_id: case.probe_outbound_id.clone(),
            details: case.details,
            probe: case.probe,
        }
    }

    fn verify_scope(&self, scope: &CoreScope) -> Result<(), PortError> {
        if scope == &self.expected_scope {
            Ok(())
        } else {
            Err(PortError::Rejected(Failure::new(
                ErrorCode::InvalidState,
                "fake outbound adapter received a foreign or stale scope",
                false,
            )))
        }
    }

    fn verify_revision(&self, expected: SnapshotRevision) -> Result<(), PortError> {
        let actual = *self.revision.lock().expect("fake inventory revision lock");
        if expected == SnapshotRevision::new(actual) {
            Ok(())
        } else {
            Err(PortError::Rejected(Failure::new(
                ErrorCode::InvalidState,
                "fake outbound adapter rejected a stale inventory revision",
                false,
            )))
        }
    }

    fn revision_value(&self) -> u64 {
        *self.revision.lock().expect("fake inventory revision lock")
    }
}

#[async_trait]
impl OutboundPort for FakeOutbound {
    async fn inventory(&self, scope: CoreScope) -> Result<OutboundInventorySnapshot, PortError> {
        self.verify_scope(&scope)?;
        let selected = self
            .selected
            .lock()
            .expect("fake selected member lock")
            .clone();
        let revision = self.revision_value();
        OutboundInventorySnapshot::new(
            scope,
            SnapshotRevision::new(revision),
            OutboundInventoryState::Ready {
                entries: self
                    .detail_ids
                    .iter()
                    .map(|id| OutboundSnapshot {
                        id: id.clone(),
                        display_name: Some(id.value().to_owned()),
                    })
                    .collect(),
                collections: vec![OutboundSelectionSet {
                    id: self.collection_id.clone(),
                    member_ids: self.member_ids.clone(),
                    availability: SelectionAvailability::Selectable,
                    selection: OutboundSelectionState::Ready { selected },
                }],
            },
        )
        .map_err(|error| PortError::Failed(error.to_string()))
    }

    async fn details(
        &self,
        scope: CoreScope,
        expected_revision: SnapshotRevision,
    ) -> Result<OutboundDetailsSnapshot, PortError> {
        self.verify_scope(&scope)?;
        self.verify_revision(expected_revision)?;
        let state = match self.details {
            DetailsBehavior::Ready => OutboundDetailsState::Ready {
                details: self.detail_ids.iter().map(fake_detail).collect(),
            },
            DetailsBehavior::Unsupported => OutboundDetailsState::Unsupported,
        };
        OutboundDetailsSnapshot::new(scope, expected_revision, state)
            .map_err(|error| PortError::Failed(error.to_string()))
    }

    async fn select(
        &self,
        scope: CoreScope,
        expected_revision: SnapshotRevision,
        collection_id: OutboundId,
        outbound_id: OutboundId,
    ) -> Result<OutboundInventorySnapshot, PortError> {
        self.verify_scope(&scope)?;
        self.verify_revision(expected_revision)?;
        if collection_id != self.collection_id || !self.member_ids.contains(&outbound_id) {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::InvalidState,
                "fake selector request was not in its current inventory",
                false,
            )));
        }
        *self.selected.lock().expect("fake selected member lock") = outbound_id;
        {
            let mut revision = self.revision.lock().expect("fake inventory revision lock");
            *revision = revision
                .checked_add(1)
                .ok_or_else(|| PortError::Failed("fake revision exhausted".to_owned()))?;
        }
        self.inventory(scope).await
    }

    async fn probe(
        &self,
        scope: CoreScope,
        expected_revision: SnapshotRevision,
        outbound_id: OutboundId,
        _request: OutboundProbeRequest,
    ) -> Result<OutboundProbeSnapshot, PortError> {
        self.verify_scope(&scope)?;
        self.verify_revision(expected_revision)?;
        if outbound_id != self.probe_outbound_id || !self.detail_ids.contains(&outbound_id) {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::InvalidState,
                "fake probe target was not registered",
                false,
            )));
        }
        let state = match self.probe {
            ProbeBehavior::Measured => OutboundProbeState::Measured { delay_ms: 1 },
            ProbeBehavior::Unsupported => OutboundProbeState::Unsupported,
        };
        OutboundProbeSnapshot::new(
            scope,
            expected_revision,
            expected_revision,
            outbound_id,
            state,
        )
        .map_err(|error| PortError::Failed(error.to_string()))
    }
}

fn fake_detail(id: &OutboundId) -> OutboundDetail {
    OutboundDetail {
        id: id.clone(),
        protocol: OutboundProtocolObservation::Unknown,
        protocol_display_name: None,
        cipher: OutboundObservation::Unknown,
        cipher_display_name: None,
        collection_kind: OutboundObservation::Unknown,
        features: OutboundObservation::Unknown,
        endpoint: OutboundObservation::Unknown,
        udp: OutboundUdpSupport::Unknown,
        health: OutboundHealthObservation::Unknown,
        current_latency_ms: OutboundObservation::Unknown,
        latency_history: OutboundObservation::Unknown,
        rtt: OutboundObservation::Unknown,
        historical_egress: OutboundObservation::Unknown,
        probe_availability: OutboundProbeAvailability::Unknown,
    }
}

/// In-memory routing intent adapter for one case and scope.
pub struct FakeRouting {
    expected_scope: CoreScope,
    behavior: RoutingBehavior,
    current: Mutex<RoutingIntent>,
}

impl FakeRouting {
    pub fn new(scope: CoreScope, behavior: RoutingBehavior) -> Self {
        let current = match &behavior {
            RoutingBehavior::Writable { initial, .. }
            | RoutingBehavior::ReadOnlyUnsupported { initial, .. } => initial.clone(),
        };
        Self {
            expected_scope: scope,
            behavior,
            current: Mutex::new(current),
        }
    }

    fn verify_scope(&self, scope: &CoreScope) -> Result<(), PortError> {
        if scope == &self.expected_scope {
            Ok(())
        } else {
            Err(PortError::Rejected(Failure::new(
                ErrorCode::InvalidState,
                "fake routing adapter received a foreign or stale scope",
                false,
            )))
        }
    }
}

#[async_trait]
impl RoutingIntentPort for FakeRouting {
    async fn observe(&self, scope: &CoreScope) -> Result<RoutingIntent, PortError> {
        self.verify_scope(scope)?;
        Ok(self
            .current
            .lock()
            .expect("fake routing state lock")
            .clone())
    }

    async fn apply(
        &self,
        scope: &CoreScope,
        intent: RoutingIntent,
    ) -> Result<RoutingIntent, PortError> {
        self.verify_scope(scope)?;
        match &self.behavior {
            RoutingBehavior::Writable { initial, requested }
                if &intent == initial || &intent == requested =>
            {
                *self.current.lock().expect("fake routing state lock") = intent.clone();
                Ok(intent)
            }
            RoutingBehavior::ReadOnlyUnsupported { .. } => Err(PortError::unsupported(
                Capability::RoutingIntent,
                "fake read-only backend preserves its engine-defined intent",
            )),
            RoutingBehavior::Writable { .. } => Err(PortError::Rejected(Failure::new(
                ErrorCode::InvalidState,
                "fake routing adapter received an unregistered intent",
                false,
            ))),
        }
    }
}

/// In-memory switch journal with `EngineId` selections.
///
/// This is a thin, public third-party fixture over the runtime's deterministic
/// journal, so the conformance kit can exercise the whole coordinator protocol
/// without a durable store.
#[derive(Clone)]
pub struct InMemoryEngineSwitchJournal {
    journal: MemoryEngineSwitchJournal,
}

impl InMemoryEngineSwitchJournal {
    pub fn new(selection: EngineId) -> Self {
        Self {
            journal: MemoryEngineSwitchJournal::new(selection),
        }
    }
}

#[async_trait]
impl EngineSwitchJournal for InMemoryEngineSwitchJournal {
    type Selection = EngineId;

    async fn snapshot(&self) -> Result<EngineSwitchSnapshot<EngineId>, PortError> {
        self.journal.snapshot().await
    }

    async fn bind_owner(
        &self,
        expected_revision: EngineSwitchRevision,
        selection: EngineId,
        identity: CoreIdentity,
        generation: CoreGeneration,
    ) -> Result<(EngineSwitchSnapshot<EngineId>, Box<dyn EngineOwnerLease>), PortError> {
        self.journal
            .bind_owner(expected_revision, selection, identity, generation)
            .await
    }

    async fn begin_owner_switch(
        &self,
        expected_revision: EngineSwitchRevision,
        previous: EngineId,
        owner_identity: CoreIdentity,
        owner_generation: CoreGeneration,
    ) -> Result<Box<dyn EngineOwnerSwitchLease<EngineId>>, PortError> {
        self.journal
            .begin_owner_switch(
                expected_revision,
                previous,
                owner_identity,
                owner_generation,
            )
            .await
    }

    async fn recover_owner_switch(
        &self,
        expected_revision: EngineSwitchRevision,
    ) -> Result<EngineSwitchRecovery<EngineId>, PortError> {
        self.journal.recover_owner_switch(expected_revision).await
    }

    async fn recover_old_owner_after_terminal(
        &self,
        expected_revision: EngineSwitchRevision,
        terminal_generation: CoreGeneration,
    ) -> Result<Box<dyn EngineOwnerStoppedSwitchLease<EngineId>>, PortError> {
        self.journal
            .recover_old_owner_after_terminal(expected_revision, terminal_generation)
            .await
    }

    async fn recover_candidate_after_terminal(
        &self,
        expected_revision: EngineSwitchRevision,
        terminal_generation: CoreGeneration,
    ) -> Result<EngineSwitchSnapshot<EngineId>, PortError> {
        self.journal
            .recover_candidate_after_terminal(expected_revision, terminal_generation)
            .await
    }
}
