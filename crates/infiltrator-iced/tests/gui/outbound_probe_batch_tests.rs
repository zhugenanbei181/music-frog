//! test-intent: behavior
//! Typed outbound batch requests stay bound to accepted inventory and readback.
use crate::state::AppState;
use crate::state::proxy_probe::OutboundProbeBatchState;
use crate::test_mounts::command_harness::{
    RecordingHandler, install_test_core_capabilities, recording_application,
};
use crate::test_mounts::outbound_fixtures::terminal;
use crate::types::message::Message;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::core_capabilities::{
    CoreCapabilitySnapshot, CoreOperation, OutboundOperation,
};
use infiltrator_contract::core_identity::CoreScope;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::outbound::{
    details::{
        OutboundCollectionKind, OutboundDetail, OutboundDetailsSnapshot, OutboundDetailsState,
        OutboundEndpoint, OutboundHealthObservation, OutboundObservation,
        OutboundProbeAvailability, OutboundProtocolObservation, OutboundRttStatistics,
        OutboundSnapshot, OutboundUdpSupport,
    },
    identity::{OutboundCipherId, OutboundId, OutboundProtocolId, SnapshotRevision},
    inventory::{
        OutboundInventorySnapshot, OutboundInventoryState, OutboundSelectionSet,
        OutboundSelectionState, SelectionAvailability,
    },
    preferences::{OutboundUiPreferencesSnapshot, OutboundUiPreferencesValues},
    probe::{
        OutboundProbeBatchItem, OutboundProbeBatchOutcome, OutboundProbeBatchScope,
        OutboundProbeBatchSnapshot, OutboundProbeConcurrency, ProbeOutboundBatchRequest,
    },
};
use infiltrator_contract::runtime_settings::{RuntimeProbeSettings, RuntimeProbeSettingsState};
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::{PageData, SurfaceSnapshot};
use std::sync::Arc;

const INVENTORY_REVISION: u64 = 41;
const DETAILS_REVISION: u64 = 9;

struct BatchFixture {
    scope: CoreScope,
    inventory: OutboundInventorySnapshot,
    details: OutboundDetailsSnapshot,
    preferences: OutboundUiPreferencesSnapshot,
    collection_id: OutboundId,
    expected_collection_members: Vec<OutboundId>,
    expected_all_members: Vec<OutboundId>,
}

impl BatchFixture {
    fn new(scope: &CoreScope) -> Self {
        let engine = scope.identity.engine.clone();
        let collection_id = OutboundId::new(engine.clone(), "GLOBAL").unwrap();
        let nested_collection = OutboundId::new(engine.clone(), "CHILD").unwrap();
        let leaf_a = OutboundId::new(engine.clone(), "leaf-a").unwrap();
        let leaf_b = OutboundId::new(engine.clone(), "leaf-b").unwrap();
        let leaf_c = OutboundId::new(engine.clone(), "leaf-c").unwrap();
        let entries = [
            (&collection_id, "Global collection"),
            (&nested_collection, "Nested collection"),
            (&leaf_a, "Leaf A"),
            (&leaf_b, "Leaf B"),
            (&leaf_c, "Leaf C"),
        ]
        .into_iter()
        .map(|(id, display_name)| OutboundSnapshot {
            id: id.clone(),
            display_name: Some(display_name.to_owned()),
        })
        .collect();
        let inventory = OutboundInventorySnapshot::new(
            scope.clone(),
            SnapshotRevision::new(INVENTORY_REVISION),
            OutboundInventoryState::Ready {
                entries,
                collections: vec![
                    OutboundSelectionSet {
                        id: collection_id.clone(),
                        member_ids: vec![nested_collection.clone(), leaf_a.clone()],
                        availability: SelectionAvailability::Selectable,
                        selection: OutboundSelectionState::Unknown,
                    },
                    OutboundSelectionSet {
                        id: nested_collection.clone(),
                        member_ids: vec![leaf_b.clone(), leaf_c.clone()],
                        availability: SelectionAvailability::ReadOnly,
                        selection: OutboundSelectionState::Unknown,
                    },
                ],
            },
        )
        .unwrap();
        let details = OutboundDetailsSnapshot::new(
            scope.clone(),
            SnapshotRevision::new(DETAILS_REVISION),
            OutboundDetailsState::Ready {
                details: [
                    (&collection_id, true),
                    (&nested_collection, true),
                    (&leaf_a, false),
                    (&leaf_b, false),
                    (&leaf_c, false),
                ]
                .into_iter()
                .map(|(id, is_collection)| OutboundDetail {
                    id: id.clone(),
                    protocol: OutboundProtocolObservation::Reported(
                        OutboundProtocolId::new(engine.clone(), "shadowsocks").unwrap(),
                    ),
                    protocol_display_name: Some("Shadowsocks".into()),
                    cipher: OutboundObservation::Observed(
                        OutboundCipherId::new(engine.clone(), "chacha20-ietf-poly1305").unwrap(),
                    ),
                    cipher_display_name: Some("ChaCha20-Poly1305".into()),
                    collection_kind: if is_collection {
                        OutboundObservation::Observed(OutboundCollectionKind::ManualSelection)
                    } else {
                        OutboundObservation::Unknown
                    },
                    features: OutboundObservation::Unknown,
                    endpoint: OutboundObservation::Observed(OutboundEndpoint {
                        host: "192.0.2.1".into(),
                        port: OutboundObservation::Unknown,
                    }),
                    udp: OutboundUdpSupport::Unknown,
                    health: OutboundHealthObservation::Unknown,
                    current_latency_ms: OutboundObservation::Unknown,
                    latency_history: OutboundObservation::Unknown,
                    rtt: OutboundObservation::Observed(OutboundRttStatistics::default()),
                    historical_egress: OutboundObservation::Unknown,
                    probe_availability: OutboundProbeAvailability::Supported,
                })
                .collect(),
            },
        )
        .unwrap();
        let preferences = OutboundUiPreferencesSnapshot::new(
            scope.clone(),
            SnapshotRevision::new(1),
            OutboundUiPreferencesValues {
                search_query: String::new(),
                favorite_outbound_ids: Vec::new(),
                group_order: Vec::new(),
                collapsed_group_ids: Vec::new(),
                sort_order: Default::default(),
                health_filter: Default::default(),
                layout: Default::default(),
            },
        )
        .unwrap();
        Self {
            scope: scope.clone(),
            inventory,
            details,
            preferences,
            collection_id,
            expected_collection_members: vec![leaf_a.clone()],
            expected_all_members: vec![leaf_a, leaf_b, leaf_c],
        }
    }

    fn inventory_at_revision(&self, revision: u64) -> OutboundInventorySnapshot {
        let OutboundInventoryState::Ready {
            entries,
            collections,
        } = self.inventory.inventory()
        else {
            unreachable!()
        };
        OutboundInventorySnapshot::new(
            self.scope.clone(),
            SnapshotRevision::new(revision),
            OutboundInventoryState::Ready {
                entries: entries.clone(),
                collections: collections.clone(),
            },
        )
        .unwrap()
    }

    fn attach(&self, state: &mut AppState, application: &CoreApplication) {
        let core = application.snapshot();
        state.attach_product_application(application.clone());
        state.shell.demo = false;
        let mut surface = SurfaceSnapshot::unavailable(
            SurfaceKind::IcedDesktop,
            HostKind::Desktop,
            Failure::unsupported("unrelated test page"),
        );
        surface.core = core;
        surface.generation = surface.core.generation;
        surface.revision = 1;
        let matrix = application
            .core_capability_application()
            .matrix_for(
                &self.scope.identity.engine,
                self.scope.identity.version.as_ref(),
            )
            .unwrap();
        surface.core_capability_snapshot =
            PageData::ready(CoreCapabilitySnapshot::from_matrix(&matrix));
        surface.outbound_inventory = PageData::ready(self.inventory.clone());
        surface.outbound_details = PageData::ready(self.details.clone());
        surface.outbound_ui_preferences = PageData::ready(self.preferences.clone());
        assert!(state.apply_shared_surface_snapshot(surface));
        state.runtime.runtime_probe_settings_editor.observe(
            &RuntimeProbeSettingsState::Available {
                settings: RuntimeProbeSettings::default(),
            },
        );
    }
}

fn product_state() -> (AppState, Arc<RecordingHandler>, BatchFixture) {
    let (application, handler) = recording_application();
    install_test_core_capabilities(
        &application,
        [CoreOperation::Outbound(OutboundOperation::Probe)],
    );
    let scope = application
        .snapshot()
        .scope()
        .expect("identified test owner");
    let fixture = BatchFixture::new(&scope);
    let mut state = AppState::empty();
    fixture.attach(&mut state, &application);
    (state, handler, fixture)
}

fn request(target: OutboundProbeBatchScope) -> ProbeOutboundBatchRequest {
    let options = RuntimeProbeSettings::default();
    ProbeOutboundBatchRequest::new(
        target,
        SnapshotRevision::new(INVENTORY_REVISION),
        options.url(),
        u64::from(options.timeout_ms()),
        OutboundProbeConcurrency::new(30).unwrap(),
    )
    .unwrap()
}

fn batch_result(
    fixture: &BatchFixture,
    request: ProbeOutboundBatchRequest,
    ids: &[OutboundId],
) -> OutboundProbeBatchSnapshot {
    OutboundProbeBatchSnapshot::new(
        fixture.scope.clone(),
        request,
        SnapshotRevision::new(DETAILS_REVISION + 1),
        ids.iter()
            .enumerate()
            .map(|(index, id)| {
                let outcome = match index {
                    0 => OutboundProbeBatchOutcome::Measured { delay_ms: 10 },
                    1 => OutboundProbeBatchOutcome::TimedOut,
                    _ => OutboundProbeBatchOutcome::Unsupported,
                };
                OutboundProbeBatchItem {
                    outbound_id: id.clone(),
                    outcome,
                }
            })
            .collect(),
    )
    .unwrap()
}

fn run_batch(
    state: &mut AppState,
    handler: &RecordingHandler,
    fixture: &BatchFixture,
    target_scope: OutboundProbeBatchScope,
    expected_ids: &[OutboundId],
) {
    let request = request(target_scope.clone());
    handler
        .2
        .lock()
        .unwrap()
        .push_back(CommandOutput::OutboundProbeBatch(batch_result(
            fixture,
            request.clone(),
            expected_ids,
        )));
    let message = state
        .outbound_probe_batch_message(target_scope)
        .expect("accepted inventory produces a scoped batch intent");
    let finished = terminal(state.update(message));
    let _ = state.update(finished);
    let commands = handler.0.lock().unwrap();
    assert!(matches!(
        commands.last(),
        Some(CommandIntent::ProbeOutboundBatch { request: actual }) if actual == &request
    ));
}

#[test]
fn collection_batch_probes_direct_concrete_members_only() {
    let (mut state, handler, fixture) = product_state();
    run_batch(
        &mut state,
        &handler,
        &fixture,
        OutboundProbeBatchScope::Collection(fixture.collection_id.clone()),
        &fixture.expected_collection_members,
    );
    let OutboundProbeBatchState::Ready { snapshot, .. } = &state.runtime.outbound_probe_batch
    else {
        panic!("typed collection result was not accepted");
    };
    assert_eq!(
        snapshot
            .items()
            .iter()
            .map(|item| item.outbound_id.clone())
            .collect::<Vec<_>>(),
        fixture.expected_collection_members
    );
}

#[test]
fn selected_batch_preserves_explicit_target_order() {
    let (mut state, handler, fixture) = product_state();
    let selected = vec![
        fixture.expected_all_members[2].clone(),
        fixture.expected_all_members[0].clone(),
    ];
    run_batch(
        &mut state,
        &handler,
        &fixture,
        OutboundProbeBatchScope::Selected(selected.clone()),
        &selected,
    );
    let OutboundProbeBatchState::Ready { snapshot, .. } = &state.runtime.outbound_probe_batch
    else {
        panic!("typed selected-target result was not accepted");
    };
    assert_eq!(
        snapshot
            .items()
            .iter()
            .map(|item| item.outbound_id.clone())
            .collect::<Vec<_>>(),
        selected
    );
}

#[test]
fn selected_collection_target_is_rejected_before_command_submission() {
    let (state, handler, fixture) = product_state();
    assert!(
        state
            .outbound_probe_batch_message(OutboundProbeBatchScope::Selected(vec![
                fixture.collection_id,
            ]))
            .is_none()
    );
    assert!(handler.0.lock().unwrap().is_empty());
}

#[test]
fn all_batch_probes_every_concrete_inventory_entry() {
    let (mut state, handler, fixture) = product_state();
    run_batch(
        &mut state,
        &handler,
        &fixture,
        OutboundProbeBatchScope::All,
        &fixture.expected_all_members,
    );
    let OutboundProbeBatchState::Ready { snapshot, .. } = &state.runtime.outbound_probe_batch
    else {
        panic!("typed all-outbounds result was not accepted");
    };
    assert_eq!(
        snapshot
            .items()
            .iter()
            .map(|item| item.outbound_id.clone())
            .collect::<Vec<_>>(),
        fixture.expected_all_members
    );
    let outcomes = snapshot
        .items()
        .iter()
        .map(|item| &item.outcome)
        .collect::<Vec<_>>();
    assert!(matches!(
        outcomes.as_slice(),
        [
            OutboundProbeBatchOutcome::Measured { delay_ms: 10 },
            OutboundProbeBatchOutcome::TimedOut,
            OutboundProbeBatchOutcome::Unsupported,
        ]
    ));
}

#[test]
fn stale_inventory_revision_rejects_completed_batch_readback() {
    let (mut state, handler, fixture) = product_state();
    let request = request(OutboundProbeBatchScope::All);
    handler
        .2
        .lock()
        .unwrap()
        .push_back(CommandOutput::OutboundProbeBatch(batch_result(
            &fixture,
            request,
            &fixture.expected_all_members,
        )));
    let start = state
        .outbound_probe_batch_message(OutboundProbeBatchScope::All)
        .unwrap();
    let finished = terminal(state.update(start));
    state.apply_outbound_inventory_observation(PageData::ready(
        fixture.inventory_at_revision(INVENTORY_REVISION + 1),
    ));
    let _ = state.update(finished);
    assert!(matches!(
        &state.runtime.outbound_probe_batch,
        OutboundProbeBatchState::Failed { failure, .. }
            if failure.message.contains("stale")
    ));
}

#[test]
fn dismissing_batch_discards_a_late_receipt() {
    let (mut state, handler, fixture) = product_state();
    let request = request(OutboundProbeBatchScope::All);
    handler
        .2
        .lock()
        .unwrap()
        .push_back(CommandOutput::OutboundProbeBatch(batch_result(
            &fixture,
            request,
            &fixture.expected_all_members,
        )));
    let start = state
        .outbound_probe_batch_message(OutboundProbeBatchScope::All)
        .unwrap();
    let finished = terminal(state.update(start));
    let _ = state.update(Message::DismissOutboundProbeBatch);
    let _ = state.update(finished);
    assert_eq!(
        state.runtime.outbound_probe_batch,
        OutboundProbeBatchState::Idle
    );
}

#[test]
fn batch_failure_stays_visible_and_can_be_retried_or_closed() {
    let (mut state, handler, _) = product_state();
    *handler.1.lock().unwrap() = Some(Failure::new(
        ErrorCode::NotReady,
        "probe adapter is temporarily unavailable",
        true,
    ));
    let start = state
        .outbound_probe_batch_message(OutboundProbeBatchScope::All)
        .unwrap();
    let finished = terminal(state.update(start));
    let _ = state.update(finished);
    assert!(matches!(
        &state.runtime.outbound_probe_batch,
        OutboundProbeBatchState::Failed { failure, .. }
            if failure.message == "probe adapter is temporarily unavailable"
    ));
    assert!(
        state
            .outbound_probe_batch_message(OutboundProbeBatchScope::All)
            .is_some()
    );
    let _ = state.update(Message::DismissOutboundProbeBatch);
    assert_eq!(
        state.runtime.outbound_probe_batch,
        OutboundProbeBatchState::Idle
    );
}
