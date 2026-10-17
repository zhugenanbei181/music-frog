//! test-intent: behavior
use crate::command_harness::{install_test_core_capabilities, recording_application};
use crate::outbound_fixtures::{OutboundFixture, OutboundFixtureInput, standard_members};
use crate::test_support::demo_env;
use futures_util::StreamExt;
use iced::Task;
use iced_runtime::Action;
use iced_runtime::task::into_stream;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::core_capabilities::{CoreOperation, OutboundOperation};
use infiltrator_contract::runtime_settings::{
    DEFAULT_RUNTIME_PROBE_URL, RuntimeProbeSettings, RuntimeProbeSettingsState,
};
use infiltrator_contract::shortcuts::KeyModifiers;
use infiltrator_contract::surface_snapshot::PageData;
use infiltrator_iced::state::AppState;
use infiltrator_iced::state::proxy_probe;
use infiltrator_iced::types::app::Route;
use infiltrator_iced::types::message::Message;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::outbound::details::OutboundObservation;
use lurker_core_contract::outbound::details::OutboundProbeAvailability;
use lurker_core_contract::outbound::identity::OutboundId;
use lurker_core_contract::outbound::preferences::OutboundHealthFilter;
use lurker_core_contract::outbound::preferences::OutboundLayout;
use lurker_core_contract::outbound::preferences::OutboundSortOrder;
use lurker_core_contract::outbound::{
    details::{OutboundDetailsSnapshot, OutboundDetailsState},
    identity::SnapshotRevision,
    inventory::{
        OutboundInventorySnapshot, OutboundInventoryState, OutboundProbeSnapshot,
        OutboundProbeState, OutboundSelectionState,
    },
};
use tokio::runtime::Builder;

fn state() -> AppState {
    AppState::demo(&demo_env(Route::Proxies)).0
}

fn demo_outbound_id(state: &AppState, name: &str) -> OutboundId {
    state
        .runtime
        .outbound_inventory
        .data
        .as_ref()
        .and_then(|inventory| match inventory.inventory() {
            OutboundInventoryState::Ready { entries, .. } => entries
                .iter()
                .find(|entry| entry.id.value() == name)
                .map(|entry| entry.id.clone()),
            _ => None,
        })
        .expect("typed demo inventory contains requested outbound")
}
fn terminal(task: Task<Message>) -> Message {
    Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let mut stream = into_stream(task).expect("tracked probe task");
            let Some(Action::Output(message)) = stream.next().await else {
                panic!("probe must deliver a terminal message")
            };
            assert!(stream.next().await.is_none());
            message
        })
}

#[test]
fn native_inspection_uses_shared_observations_and_close_escape_and_navigation_never_select_nodes() {
    let mut state = state();
    let outbound_id = demo_outbound_id(&state, "HK-01");
    let inventory = state.runtime.outbound_inventory.clone();
    assert_eq!(
        state
            .update(Message::InspectOutbound(Some(outbound_id.clone())))
            .units(),
        0
    );
    assert_eq!(
        state.runtime.outbound_inspecting.as_ref(),
        Some(&outbound_id)
    );
    assert!(
        state
            .runtime
            .outbound_details
            .data
            .as_ref()
            .and_then(|details| details.detail(&outbound_id))
            .is_some()
    );
    assert_eq!(state.update(Message::InspectOutbound(None)).units(), 0);
    assert!(state.runtime.outbound_inspecting.is_none());
    let _ = state.update(Message::InspectOutbound(Some(outbound_id.clone())));
    assert_eq!(
        state
            .update(Message::KeyboardChord {
                key: "Escape".into(),
                modifiers: KeyModifiers::default()
            })
            .units(),
        0
    );
    assert!(state.runtime.outbound_inspecting.is_none());
    let _ = state.update(Message::InspectOutbound(Some(outbound_id.clone())));
    let _ = state.update(Message::Navigate(Route::Profiles));
    assert!(state.runtime.outbound_inspecting.is_none());
    assert_eq!(state.runtime.outbound_inventory, inventory);
}

#[test]
fn native_inspection_follows_stable_identity_on_refresh_and_rejects_a_removed_node() {
    let mut state = state();
    let outbound_id = demo_outbound_id(&state, "HK-01");
    let _ = state.update(Message::InspectOutbound(Some(outbound_id.clone())));
    let scope = state
        .surface
        .latest()
        .and_then(|snapshot| snapshot.core.scope())
        .expect("typed demo core scope");
    let mut surface = state.surface.latest().unwrap().clone();
    let current_details = state
        .runtime
        .outbound_details
        .data
        .as_ref()
        .expect("demo details")
        .clone();
    let OutboundDetailsState::Ready { mut details } = current_details.state().clone() else {
        panic!("demo details are ready")
    };
    details
        .iter_mut()
        .find(|detail| detail.id == outbound_id)
        .expect("inspected outbound details")
        .current_latency_ms = OutboundObservation::Observed(91);
    surface.revision += 1;
    surface.outbound_details = PageData::ready(
        OutboundDetailsSnapshot::new(
            scope.clone(),
            SnapshotRevision::new(current_details.detail_revision().value() + 1),
            OutboundDetailsState::Ready { details },
        )
        .expect("valid updated detail observation"),
    );
    assert!(state.apply_shared_surface_snapshot(surface));
    assert_eq!(
        state.runtime.outbound_inspecting.as_ref(),
        Some(&outbound_id)
    );
    assert_eq!(
        state
            .runtime
            .outbound_details
            .data
            .as_ref()
            .and_then(|details| details.detail(&outbound_id))
            .cloned()
            .unwrap()
            .current_latency_ms,
        OutboundObservation::Observed(91)
    );

    let current_inventory = state.runtime.outbound_inventory.data.as_ref().unwrap();
    let OutboundInventoryState::Ready {
        mut entries,
        mut collections,
    } = current_inventory.inventory().clone()
    else {
        panic!("demo inventory is ready")
    };
    entries.retain(|entry| entry.id != outbound_id);
    for collection in &mut collections {
        collection.member_ids.retain(|id| id != &outbound_id);
        if matches!(
            &collection.selection,
            OutboundSelectionState::Ready { selected } if selected == &outbound_id
        ) {
            collection.selection = collection
                .member_ids
                .first()
                .cloned()
                .map(|selected| OutboundSelectionState::Ready { selected })
                .unwrap_or(OutboundSelectionState::Empty);
        }
    }
    let next_inventory_revision = current_inventory.revision().value() + 1;
    let next_inventory = OutboundInventorySnapshot::new(
        scope.clone(),
        SnapshotRevision::new(next_inventory_revision),
        OutboundInventoryState::Ready {
            entries,
            collections,
        },
    )
    .expect("valid inventory without removed outbound");
    let mut surface = state.surface.latest().unwrap().clone();
    let current_details = state.runtime.outbound_details.data.as_ref().unwrap();
    let OutboundDetailsState::Ready { mut details } = current_details.state().clone() else {
        panic!("demo details are ready")
    };
    details.retain(|detail| detail.id != outbound_id);
    surface.revision += 1;
    surface.outbound_inventory = PageData::ready(next_inventory);
    surface.outbound_details = PageData::ready(
        OutboundDetailsSnapshot::new(
            scope,
            SnapshotRevision::new(current_details.detail_revision().value() + 1),
            OutboundDetailsState::Ready { details },
        )
        .expect("valid details without removed outbound"),
    );
    assert!(state.apply_shared_surface_snapshot(surface));
    assert_eq!(
        state
            .update(Message::InspectOutbound(Some(outbound_id)))
            .units(),
        0
    );
    assert!(state.runtime.outbound_inspecting.is_none());
}

#[test]
fn native_probe_preserves_failure_retries_the_same_intent_and_ignores_feedback_after_close() {
    let (mut state, _) = AppState::new();
    let (application, handler) = recording_application();
    install_test_core_capabilities(
        &application,
        [CoreOperation::Outbound(OutboundOperation::Probe)],
    );
    let scope = application
        .snapshot()
        .scope()
        .expect("identified owner scope");
    let fixture = OutboundFixture::new(OutboundFixtureInput {
        scope: &scope,
        revision: 51,
        members: &standard_members(),
        selected: Some("proxy-a"),
        favorites: &[],
        health_filter: OutboundHealthFilter::All,
        sort_order: OutboundSortOrder::LatencyAscending,
        layout: OutboundLayout::Grid,
    });
    fixture.attach_to(&mut state, &application);
    state
        .runtime
        .runtime_probe_settings_editor
        .observe(&RuntimeProbeSettingsState::Available {
            settings: RuntimeProbeSettings::default(),
        });
    let outbound_id = fixture.ids["proxy-a"].clone();
    let _ = state.update(Message::InspectOutbound(Some(outbound_id.clone())));
    let failure = Failure::new(ErrorCode::Permission, "grant proxy probe permission", true);
    *handler.1.lock().unwrap() = Some(failure.clone());
    let task = state.update(Message::ProbeInspectedOutbound);
    assert_eq!(state.update(Message::ProbeInspectedOutbound).units(), 0);
    assert!(state.runtime.outbound_probe_pending.is_some());
    let result = terminal(task);
    assert_eq!(state.update(result).units(), 0);
    assert_eq!(state.runtime.outbound_probe_failure, Some(failure));
    assert_eq!(
        state.runtime.outbound_inspecting.as_ref(),
        Some(&outbound_id)
    );
    *handler.1.lock().unwrap() = None;
    let measured = OutboundProbeSnapshot::new(
        scope.clone(),
        SnapshotRevision::new(51),
        SnapshotRevision::new(52),
        outbound_id.clone(),
        OutboundProbeState::Measured { delay_ms: 77 },
    )
    .unwrap();
    handler
        .2
        .lock()
        .unwrap()
        .push_back(CommandOutput::OutboundProbe(measured));
    let task = state.update(Message::ProbeInspectedOutbound);
    let result = terminal(task);
    assert_eq!(state.update(result).units(), 0);
    assert!(state.runtime.outbound_probe_blocked.is_none());
    assert!(state.runtime.outbound_probe_failure.is_none());
    assert_eq!(
        state
            .runtime
            .outbound_probe_result
            .as_ref()
            .map(|result| result.state()),
        Some(&OutboundProbeState::Measured { delay_ms: 77 })
    );
    let measured_again = OutboundProbeSnapshot::new(
        scope,
        SnapshotRevision::new(51),
        SnapshotRevision::new(53),
        outbound_id.clone(),
        OutboundProbeState::Measured { delay_ms: 79 },
    )
    .unwrap();
    handler
        .2
        .lock()
        .unwrap()
        .push_back(CommandOutput::OutboundProbe(measured_again));
    let task = state.update(Message::ProbeInspectedOutbound);
    assert_eq!(state.update(Message::InspectOutbound(None)).units(), 0);
    let result = terminal(task);
    assert_eq!(state.update(result).units(), 0);
    assert!(state.runtime.outbound_inspecting.is_none());
    assert!(state.runtime.outbound_probe_pending.is_none());
    let commands = handler.0.lock().unwrap();
    assert_eq!(commands.len(), 3);
    let requests = commands
        .iter()
        .map(|intent| match intent {
            CommandIntent::ProbeOutbound { request } => request,
            other => panic!("unexpected probe intent: {other:?}"),
        })
        .collect::<Vec<_>>();
    assert!(requests.iter().all(|request| {
        request.outbound_id() == &outbound_id
            && request.expected_inventory_revision() == SnapshotRevision::new(51)
            && request.test_url() == DEFAULT_RUNTIME_PROBE_URL
            && request.timeout_ms() == 5000
    }));
}

#[test]
fn outbound_item_probe_support_does_not_enable_probe_without_global_core_evidence() {
    let (mut state, _) = AppState::new();
    let (application, handler) = recording_application();
    install_test_core_capabilities(
        &application,
        [
            CoreOperation::Outbound(OutboundOperation::List),
            CoreOperation::Outbound(OutboundOperation::Select),
        ],
    );
    let scope = application
        .snapshot()
        .scope()
        .expect("identified owner scope");
    let fixture = OutboundFixture::new(OutboundFixtureInput {
        scope: &scope,
        revision: 60,
        members: &standard_members(),
        selected: Some("proxy-a"),
        favorites: &[],
        health_filter: OutboundHealthFilter::All,
        sort_order: OutboundSortOrder::LatencyAscending,
        layout: OutboundLayout::Grid,
    });
    fixture.attach_to(&mut state, &application);
    state
        .runtime
        .runtime_probe_settings_editor
        .observe(&RuntimeProbeSettingsState::Available {
            settings: RuntimeProbeSettings::default(),
        });
    let outbound_id = fixture.ids["proxy-a"].clone();
    assert_eq!(
        fixture
            .details
            .detail(&outbound_id)
            .expect("typed member details")
            .probe_availability,
        OutboundProbeAvailability::Supported
    );

    let _ = state.update(Message::InspectOutbound(Some(outbound_id.clone())));
    assert_eq!(state.update(Message::ProbeInspectedOutbound).units(), 0);

    assert!(state.runtime.outbound_probe_pending.is_none());
    assert_eq!(
        state.runtime.outbound_probe_blocked,
        Some(proxy_probe::OutboundProbeActionAvailability::CoreUnknown),
        "unregistered global evidence remains a visible typed block"
    );
    assert!(state.runtime.outbound_probe_failure.is_none());
    assert!(handler.0.lock().unwrap().is_empty());
}

#[test]
fn outbound_probe_is_disabled_for_unavailable_and_explicitly_unsupported_core_evidence() {
    use infiltrator_contract::core_capabilities::{
        AdapterCapabilityDecision, AdapterRevision, CoreCapabilityEntry, CoreCapabilityMatrix,
        CoreCapabilityScope, CoreCapabilitySnapshot, EngineCapabilityDecision,
        HostCapabilityDecision,
    };
    use infiltrator_contract::surface::HostKind;

    let (mut state, _) = AppState::new();
    let (application, handler) = recording_application();
    let scope = application
        .snapshot()
        .scope()
        .expect("identified owner scope");
    let fixture = OutboundFixture::new(OutboundFixtureInput {
        scope: &scope,
        revision: 61,
        members: &standard_members(),
        selected: Some("proxy-a"),
        favorites: &[],
        health_filter: OutboundHealthFilter::All,
        sort_order: OutboundSortOrder::LatencyAscending,
        layout: OutboundLayout::Grid,
    });
    fixture.attach_to(&mut state, &application);
    state
        .runtime
        .runtime_probe_settings_editor
        .observe(&RuntimeProbeSettingsState::Available {
            settings: RuntimeProbeSettings::default(),
        });
    let outbound_id = fixture.ids["proxy-a"].clone();
    let _ = state.update(Message::InspectOutbound(Some(outbound_id)));
    let base = state.surface.latest().unwrap().clone();

    let mut unavailable = base.clone();
    unavailable.revision += 1;
    unavailable.core_capability_snapshot = PageData::unavailable(Failure::new(
        ErrorCode::NotReady,
        "capability read is unavailable",
        true,
    ));
    assert!(state.apply_shared_surface_snapshot(unavailable));
    assert_eq!(state.update(Message::ProbeInspectedOutbound).units(), 0);
    assert_eq!(
        state.runtime.outbound_probe_blocked,
        Some(proxy_probe::OutboundProbeActionAvailability::CoreUnknown),
        "unavailable global evidence remains a visible typed block"
    );

    let matrix = CoreCapabilityMatrix::new(
        CoreCapabilityScope::new(
            scope.identity.engine.clone(),
            scope.identity.version.clone(),
            HostKind::Desktop,
            AdapterRevision::new(1),
        ),
        vec![CoreCapabilityEntry::new(
            CoreOperation::Outbound(OutboundOperation::Probe),
            EngineCapabilityDecision::Unsupported {
                reason: "probe is not verified for this engine version".into(),
            },
            HostCapabilityDecision::Supported,
            AdapterCapabilityDecision::Supported,
        )],
    )
    .expect("one explicit probe capability entry");
    let mut unsupported = base;
    unsupported.revision = state.surface.latest().unwrap().revision + 1;
    unsupported.core_capability_snapshot =
        PageData::ready(CoreCapabilitySnapshot::from_matrix(&matrix));
    assert!(state.apply_shared_surface_snapshot(unsupported));
    assert_eq!(state.update(Message::ProbeInspectedOutbound).units(), 0);
    assert_eq!(
        state.runtime.outbound_probe_blocked,
        Some(proxy_probe::OutboundProbeActionAvailability::CoreUnsupported),
        "unsupported global evidence remains a visible typed block"
    );
    assert!(state.runtime.outbound_probe_failure.is_none());
    assert!(state.runtime.outbound_probe_pending.is_none());
    assert!(handler.0.lock().unwrap().is_empty());
}

#[test]
fn typed_outbound_details_keep_failure_unknown_and_cipher_observations_without_legacy_proxy_page() {
    let (mut state, _) = AppState::new();
    let (application, _) = recording_application();
    install_test_core_capabilities(
        &application,
        [CoreOperation::Outbound(OutboundOperation::Probe)],
    );
    let scope = application
        .snapshot()
        .scope()
        .expect("identified owner scope");
    let fixture = OutboundFixture::new(OutboundFixtureInput {
        scope: &scope,
        revision: 61,
        members: &standard_members(),
        selected: Some("proxy-a"),
        favorites: &[],
        health_filter: OutboundHealthFilter::All,
        sort_order: OutboundSortOrder::LatencyAscending,
        layout: OutboundLayout::Grid,
    });
    fixture.attach_to(&mut state, &application);
    let outbound_id = fixture.ids["proxy-a"].clone();
    let mut snapshot = state.surface.latest().unwrap().clone();
    let _ = state.update(Message::InspectOutbound(Some(outbound_id.clone())));
    let observed = state
        .runtime
        .outbound_details
        .data
        .as_ref()
        .and_then(|details| details.detail(&outbound_id))
        .unwrap()
        .clone();
    assert_eq!(
        observed.cipher_display_name.as_deref(),
        Some("ChaCha20-Poly1305")
    );
    assert!(matches!(observed.cipher, OutboundObservation::Observed(_)));
    let failure = Failure::new(ErrorCode::Network, "controller read failed", true);
    for bad in [
        PageData::failed(failure.clone()),
        PageData::unavailable(failure.clone()),
        PageData::loading(),
    ] {
        snapshot.revision += 1;
        snapshot.outbound_details = bad;
        assert!(state.apply_shared_surface_snapshot(snapshot.clone()));
        assert_eq!(
            state.runtime.outbound_inspecting.as_ref(),
            Some(&outbound_id)
        );
        assert!(state.runtime.outbound_details.data.is_none());
        assert_eq!(state.update(Message::ProbeInspectedOutbound).units(), 0);
        assert!(state.runtime.outbound_probe_pending.is_none());
        assert_eq!(
            state.runtime.outbound_details.status,
            snapshot.outbound_details.status
        );
    }
    let recovered = OutboundFixture::new(OutboundFixtureInput {
        scope: &scope,
        revision: 62,
        members: &standard_members(),
        selected: Some("proxy-a"),
        favorites: &[],
        health_filter: OutboundHealthFilter::All,
        sort_order: OutboundSortOrder::LatencyAscending,
        layout: OutboundLayout::Grid,
    });
    snapshot.revision += 1;
    snapshot.outbound_details = PageData::ready(recovered.details.clone());
    assert!(state.apply_shared_surface_snapshot(snapshot.clone()));
    assert_eq!(
        state
            .runtime
            .outbound_details
            .data
            .as_ref()
            .and_then(|details| details.detail(&outbound_id))
            .unwrap()
            .cipher_display_name
            .as_deref(),
        Some("ChaCha20-Poly1305")
    );
    assert!(state.runtime.outbound_probe_failure.is_none());
    assert_ne!(
        state.runtime.outbound_probe_blocked,
        Some(proxy_probe::OutboundProbeActionAvailability::DetailsUnavailable),
        "the recovered detail clears the stale details-unavailable block"
    );
    let empty_inventory = OutboundInventorySnapshot::new(
        scope.clone(),
        SnapshotRevision::new(63),
        OutboundInventoryState::Empty,
    )
    .unwrap();
    let empty_details = OutboundDetailsSnapshot::new(
        scope,
        SnapshotRevision::new(63),
        OutboundDetailsState::Empty,
    )
    .unwrap();
    snapshot.revision += 1;
    snapshot.outbound_inventory = PageData::empty(empty_inventory);
    snapshot.outbound_details = PageData::empty(empty_details);
    assert!(state.apply_shared_surface_snapshot(snapshot));
    assert!(state.runtime.outbound_inspecting.is_none());
}
