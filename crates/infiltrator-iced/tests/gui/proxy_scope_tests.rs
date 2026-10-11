//! test-intent: behavior
//! Application session changes retire legacy facts; delayed/unscoped reads cannot reintroduce them.
use crate::state::AppState;
use crate::test_mounts::command_harness::{
    RecordingHandler, install_lifecycle_test_capabilities, install_test_core_capabilities,
};
use crate::test_mounts::execute_at_observed_scope;
use crate::test_mounts::identified_core_application::identified_core_application;
use crate::test_mounts::outbound_fixtures::{
    OutboundFixture, OutboundFixtureInput, OutboundMemberFixture,
};
use crate::types::message::Message;
use infiltrator_application::connection_grouping_fixtures::grouping_snapshot;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::log_capture_fixtures::{LogCaptureProcess, populate_logs};
use infiltrator_application::surface_reader::ApplicationSurfaceReader;
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::core_capabilities::{CoreOperation, OutboundOperation};
use infiltrator_contract::core_identity::CoreScope;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::outbound::details::OutboundHealthObservation;
use infiltrator_contract::outbound::details::OutboundObservation;
use infiltrator_contract::outbound::details::OutboundProbeAvailability;
use infiltrator_contract::outbound::preferences::OutboundHealthFilter;
use infiltrator_contract::outbound::preferences::OutboundLayout;
use infiltrator_contract::outbound::preferences::OutboundSortOrder;
use infiltrator_contract::outbound::preferences::OutboundUiPreferenceIntent;
use infiltrator_contract::outbound::preferences::OutboundUiPreferencesSnapshot;
use infiltrator_contract::outbound::preferences::OutboundUiPreferencesValues;
use infiltrator_contract::outbound::{
    details::OutboundSnapshot,
    identity::{OutboundId, SnapshotRevision},
    inventory::{
        OutboundInventorySnapshot, OutboundInventoryState, OutboundSelectionSet,
        OutboundSelectionState, SelectionAvailability,
    },
    probe::OutboundSelectionRequest,
};
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::{PageData, PageStatus, SurfaceSnapshot};
use infiltrator_iced::state::proxy_preferences::ProxyPreferencePhase;
use infiltrator_iced::state::proxy_search::ProxySearchPhase;
use infiltrator_ports::surface::SurfaceReader;
use std::sync::Arc;
use tokio::runtime::Builder;

fn inventory(
    scope: &CoreScope,
    revision: u64,
    availability: SelectionAvailability,
) -> OutboundInventorySnapshot {
    let engine = scope.identity.engine.clone();
    let group = OutboundId::new(engine.clone(), "PROXY").unwrap();
    let first = OutboundId::new(engine.clone(), "node-a").unwrap();
    let second = OutboundId::new(engine, "node-b").unwrap();
    OutboundInventorySnapshot::new(
        scope.clone(),
        SnapshotRevision::new(revision),
        OutboundInventoryState::Ready {
            entries: vec![
                OutboundSnapshot {
                    id: first.clone(),
                    display_name: Some("Node A".into()),
                },
                OutboundSnapshot {
                    id: second.clone(),
                    display_name: Some("Node B".into()),
                },
            ],
            collections: vec![OutboundSelectionSet {
                id: group,
                member_ids: vec![first.clone(), second],
                availability,
                selection: OutboundSelectionState::Ready { selected: first },
            }],
        },
    )
    .unwrap()
}

fn inventory_with_two_collections(scope: &CoreScope, revision: u64) -> OutboundInventorySnapshot {
    let engine = scope.identity.engine.clone();
    let group = OutboundId::new(engine.clone(), "PROXY").unwrap();
    let second_group = OutboundId::new(engine.clone(), "SECOND").unwrap();
    let first = OutboundId::new(engine.clone(), "node-a").unwrap();
    let second = OutboundId::new(engine, "node-b").unwrap();
    OutboundInventorySnapshot::new(
        scope.clone(),
        SnapshotRevision::new(revision),
        OutboundInventoryState::Ready {
            entries: vec![
                OutboundSnapshot {
                    id: first.clone(),
                    display_name: Some("Node A".into()),
                },
                OutboundSnapshot {
                    id: second.clone(),
                    display_name: Some("Node B".into()),
                },
            ],
            collections: vec![
                OutboundSelectionSet {
                    id: group,
                    member_ids: vec![first.clone(), second.clone()],
                    availability: SelectionAvailability::Selectable,
                    selection: OutboundSelectionState::Ready {
                        selected: first.clone(),
                    },
                },
                OutboundSelectionSet {
                    id: second_group,
                    member_ids: vec![second.clone()],
                    availability: SelectionAvailability::Selectable,
                    selection: OutboundSelectionState::Ready { selected: second },
                },
            ],
        },
    )
    .unwrap()
}

fn product_state(
    inventory_page: impl FnOnce(&CoreScope) -> PageData<OutboundInventorySnapshot>,
    core_operations: impl IntoIterator<Item = CoreOperation>,
) -> (AppState, CoreApplication, CoreScope, Arc<RecordingHandler>) {
    let core = identified_core_application();
    install_test_core_capabilities(&core, core_operations);
    let handler = Arc::new(RecordingHandler::default());
    core.install_command_handler(handler.clone());
    let scope = core.snapshot().scope().expect("identified core scope");
    let mut state = AppState::empty();
    state.attach_product_application(core.clone());
    state.shell.demo = false;

    let mut snapshot = SurfaceSnapshot::unavailable(
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
        Failure::unsupported("unrelated page capability"),
    );
    snapshot.core = core.snapshot();
    snapshot.generation = snapshot.core.generation;
    snapshot.revision = 1;
    snapshot.outbound_inventory = inventory_page(&scope);
    let preference_fixture = OutboundFixture::new(OutboundFixtureInput {
        scope: &scope,
        revision: 41,
        members: &[
            OutboundMemberFixture {
                id: "node-a",
                display_name: "Node A",
                health: OutboundHealthObservation::Unknown,
                latency_ms: None,
                probe_availability: OutboundProbeAvailability::Unknown,
            },
            OutboundMemberFixture {
                id: "node-b",
                display_name: "Node B",
                health: OutboundHealthObservation::Unknown,
                latency_ms: None,
                probe_availability: OutboundProbeAvailability::Unknown,
            },
        ],
        selected: Some("node-a"),
        favorites: &[],
        health_filter: OutboundHealthFilter::All,
        sort_order: OutboundSortOrder::LatencyAscending,
        layout: OutboundLayout::Grid,
    });
    snapshot.outbound_details = PageData::unavailable(Failure::unsupported(
        "outbound details have not been observed for this test core",
    ));
    snapshot.outbound_ui_preferences = PageData::ready(preference_fixture.preferences);
    assert!(state.apply_shared_surface_snapshot(snapshot));
    (state, core, scope, handler)
}

fn outbound_preferences(
    scope: &CoreScope,
    revision: u64,
    search_query: &str,
    favorite_ids: &[&str],
    health_filter: OutboundHealthFilter,
    sort_order: OutboundSortOrder,
    layout: OutboundLayout,
) -> OutboundUiPreferencesSnapshot {
    let favorite_outbound_ids = favorite_ids
        .iter()
        .map(|value| OutboundId::new(scope.identity.engine.clone(), *value).unwrap())
        .collect();
    OutboundUiPreferencesSnapshot::new(
        scope.clone(),
        SnapshotRevision::new(revision),
        OutboundUiPreferencesValues {
            search_query: search_query.to_owned(),
            favorite_outbound_ids,
            group_order: Vec::new(),
            collapsed_group_ids: Vec::new(),
            sort_order,
            health_filter,
            layout,
        },
    )
    .unwrap()
}

fn terminal(task: iced::Task<Message>) -> Message {
    use futures_util::StreamExt;
    use iced_runtime::Action;
    use iced_runtime::task::into_stream;

    Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let mut stream = into_stream(task).expect("outbound command task");
            let Some(Action::Output(message)) = stream.next().await else {
                panic!("outbound command must deliver a terminal message")
            };
            assert!(stream.next().await.is_none());
            message
        })
}

#[test]
fn composed_snapshot_retires_demo_rows_and_rejects_reads_from_the_previous_core_session() {
    let runtime = tokio_application_runtime().unwrap();
    let process = Arc::new(LogCaptureProcess::default());
    let core = CoreApplication::new(process.clone(), process, runtime.clone());
    install_lifecycle_test_capabilities(&core);
    let producing = core.clone();
    runtime.block_on(Box::pin(async move {
        populate_logs(&producing).await.unwrap();
    }));
    let reader = ApplicationSurfaceReader::new(
        Arc::new(core.clone()),
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
    );
    let (mut state, _) = AppState::new();
    let _ = state.update(Message::ConnectionsReceived(grouping_snapshot()));
    assert_eq!(state.diag.connection_groups.source_count(), 9);
    state.attach_product_application(core.clone());
    state.shell.demo = false;
    let first_scope = core.snapshot().scope().expect("initial product scope");
    let first_inventory = inventory(&first_scope, 31, SelectionAvailability::Selectable);
    state
        .runtime
        .filtered_groups
        .push(("old-demo-group".into(), vec!["old-demo".into()]));
    let publish = |state: &mut AppState, typed_inventory: Option<OutboundInventorySnapshot>| {
        let snapshot = Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(reader.read())
            .unwrap();
        let mut snapshot = snapshot;
        if let Some(inventory) = typed_inventory {
            snapshot.outbound_inventory = PageData::ready(inventory);
        }
        assert!(state.apply_shared_surface_snapshot(snapshot));
    };
    publish(&mut state, Some(first_inventory.clone()));
    assert!(
        !state
            .runtime
            .filtered_groups
            .iter()
            .any(|(name, _)| name == "old-demo-group")
    );
    assert!(state.diag.connections.is_none());
    assert_eq!(state.diag.connection_groups.source_count(), 0);
    assert_eq!(
        state.runtime.filtered_groups,
        vec![(
            "PROXY".to_owned(),
            vec!["node-a".to_owned(), "node-b".to_owned()]
        )],
        "the product view contains only the typed inventory's outbound IDs"
    );
    assert_eq!(
        state.runtime.proxy_groups[0]
            .proxies
            .iter()
            .map(|member| member.search_name())
            .collect::<Vec<_>>(),
        ["Node A", "Node B"],
        "typed inventory display names remain visible"
    );
    assert_eq!(
        state.runtime.outbound_inventory.data.as_ref(),
        Some(&first_inventory),
        "same-session owner snapshots retain the typed outbound observation"
    );
    let restarting = core.clone();
    runtime.block_on(Box::pin(async move {
        execute_at_observed_scope(&restarting, CommandIntent::RestartCore)
            .await
            .into_unit()
            .unwrap();
    }));
    publish(&mut state, None);
    assert!(
        !state
            .runtime
            .filtered_groups
            .iter()
            .any(|(name, _)| name == "old-demo-group")
    );
    assert!(
        state
            .runtime
            .outbound_inventory
            .data
            .as_ref()
            .is_none_or(|inventory| inventory.scope() != &first_scope)
    );
    let stale_inventory = inventory(&first_scope, 32, SelectionAvailability::Selectable);
    let _ = state.update(Message::OutboundInventoryReadFinished {
        requested_scope: Some(first_scope.clone()),
        terminal_scope: Some(first_scope.clone()),
        result: Ok(stale_inventory),
    });
    assert!(
        state
            .runtime
            .outbound_inventory
            .data
            .as_ref()
            .is_none_or(|inventory| inventory.scope() != &first_scope)
    );
    let current = core.snapshot();
    let current_scope = current.scope().expect("restarted product scope");
    let current_inventory = inventory(&current_scope, 1, SelectionAvailability::Selectable);
    let mut current_surface = SurfaceSnapshot::unavailable(
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
        Failure::unsupported("unrelated surface page"),
    );
    current_surface.core = current;
    current_surface.generation = current_surface.core.generation;
    current_surface.revision = state
        .surface
        .latest()
        .map_or(1, |latest| latest.revision + 1);
    current_surface.outbound_inventory = PageData::ready(current_inventory.clone());
    assert!(state.apply_shared_surface_snapshot(current_surface));
    assert_eq!(
        state.runtime.outbound_inventory.data.as_ref(),
        Some(&current_inventory)
    );
    assert!(
        !state
            .runtime
            .filtered_groups
            .iter()
            .any(|(name, _)| name == "old-demo-group")
    );
    let closing = core.clone();
    runtime.block_on(Box::pin(async move {
        closing.close().await.unwrap();
    }));
}

#[test]
fn product_proxy_page_does_not_fall_back_to_legacy_groups_when_inventory_is_unavailable() {
    let failure = Failure::new(ErrorCode::NotReady, "inventory has not been observed", true);
    let (state, _, _, _) = product_state(
        |_| PageData::unavailable(failure.clone()),
        [CoreOperation::Outbound(OutboundOperation::Select)],
    );

    assert!(state.runtime.proxy_groups.is_empty());
    assert!(state.runtime.filtered_groups.is_empty());
    assert_eq!(
        state.runtime.outbound_inventory.status,
        PageStatus::Unavailable { failure }
    );
    assert!(state.runtime.outbound_inventory.data.is_none());
    assert!(
        state
            .outbound_selection_message("legacy-group", "legacy-node")
            .is_none()
    );
}

#[test]
fn product_proxy_selection_captures_the_accepted_scope_and_inventory_revision() {
    let (mut state, _, scope, _) = product_state(
        |scope| PageData::ready(inventory(scope, 41, SelectionAvailability::Selectable)),
        [CoreOperation::Outbound(OutboundOperation::Select)],
    );
    let group = &state.runtime.proxy_groups[0];
    assert_eq!(group.name, "PROXY");
    assert_eq!(group.current(), Some("node-a"));
    let member = group
        .proxies
        .iter()
        .find(|member| member.name == "node-b")
        .expect("typed member row");
    assert_eq!(member.display_name.as_deref(), Some("Node B"));
    assert_eq!(member.protocol_display_name, None);
    assert_eq!(member.health, OutboundHealthObservation::Unknown);
    assert_eq!(member.current_latency_ms, OutboundObservation::Unknown);
    assert_eq!(member.features, OutboundObservation::Unknown);

    let message = state
        .outbound_selection_message("PROXY", "node-b")
        .expect("selectable typed outbound");
    let Message::SelectOutbound {
        scope: captured_scope,
        request,
    } = message.clone()
    else {
        panic!("the proxy page must produce a typed outbound selection")
    };
    assert_eq!(captured_scope, scope);
    assert_eq!(request.collection_id().value(), "PROXY");
    assert_eq!(request.outbound_id().value(), "node-b");
    assert_eq!(request.expected_revision(), SnapshotRevision::new(41));

    let mut newer = state.surface.latest().unwrap().clone();
    newer.revision += 1;
    newer.outbound_inventory =
        PageData::ready(inventory(&scope, 42, SelectionAvailability::Unsupported));
    assert!(state.apply_shared_surface_snapshot(newer));
    assert_eq!(
        state.runtime.proxy_groups[0].availability,
        SelectionAvailability::Unsupported
    );
    assert!(
        state
            .outbound_selection_message("PROXY", "node-b")
            .is_none()
    );
    assert_eq!(state.update(message).units(), 0);
}

#[test]
fn product_proxy_selection_dispatches_only_the_typed_outbound_intent() {
    let (mut state, _, scope, handler) = product_state(
        |scope| PageData::ready(inventory(scope, 51, SelectionAvailability::Selectable)),
        [CoreOperation::Outbound(OutboundOperation::Select)],
    );
    *handler.1.lock().unwrap() = Some(Failure::new(
        ErrorCode::Network,
        "selection command rejected for this fixture",
        true,
    ));
    let Message::SelectOutbound {
        scope: captured_scope,
        request,
    } = state
        .outbound_selection_message("PROXY", "node-b")
        .expect("typed selection request")
    else {
        panic!("the proxy page must emit SelectOutbound")
    };
    assert_eq!(captured_scope, scope);

    let terminal = terminal(state.update(Message::SelectOutbound {
        scope: captured_scope.clone(),
        request: request.clone(),
    }));
    let Message::OutboundSelectionFinished {
        scope: result_scope,
        collection_id,
        outbound_id,
        expected_revision,
        terminal_scope,
        result,
    } = terminal
    else {
        panic!("typed selection must receive a correlated terminal")
    };
    assert_eq!(result_scope, captured_scope);
    assert_eq!(terminal_scope, Some(scope));
    assert_eq!(&collection_id, request.collection_id());
    assert_eq!(&outbound_id, request.outbound_id());
    assert_eq!(expected_revision, SnapshotRevision::new(51));
    assert!(result.is_err());
    assert!(matches!(
        handler.0.lock().unwrap().as_slice(),
        [CommandIntent::SelectOutbound { request: recorded }]
            if recorded == &request
    ));
}

#[test]
fn product_preferences_reproject_the_private_outbound_view_model() {
    let (mut state, _, scope, handler) = product_state(
        |scope| PageData::ready(inventory(scope, 61, SelectionAvailability::Selectable)),
        [],
    );

    handler
        .2
        .lock()
        .unwrap()
        .push_back(CommandOutput::OutboundUiPreferencesApplied(
            outbound_preferences(
                &scope,
                42,
                "Node B",
                &[],
                OutboundHealthFilter::All,
                OutboundSortOrder::LatencyAscending,
                OutboundLayout::Grid,
            ),
        ));
    let task = state.update(Message::FilterProxies("Node B".into()));
    assert!(matches!(
        &state.runtime.proxy_preferences.phase,
        ProxyPreferencePhase::Applying(_)
    ));
    assert!(matches!(
        &state.runtime.proxy_search.phase,
        ProxySearchPhase::Pending { query, .. } if query == "Node B"
    ));
    assert_eq!(state.update(terminal(task)).units(), 0);
    assert!(matches!(
        &state.runtime.proxy_preferences.phase,
        ProxyPreferencePhase::Idle
    ));
    assert!(matches!(
        &state.runtime.proxy_search.phase,
        ProxySearchPhase::Synchronized { query } if query == "Node B"
    ));
    assert_eq!(
        state.runtime.filtered_groups,
        vec![("PROXY".into(), vec!["node-b".into()])]
    );

    handler
        .2
        .lock()
        .unwrap()
        .push_back(CommandOutput::OutboundUiPreferencesApplied(
            outbound_preferences(
                &scope,
                43,
                "Node B",
                &["node-b"],
                OutboundHealthFilter::All,
                OutboundSortOrder::LatencyAscending,
                OutboundLayout::Grid,
            ),
        ));
    let completion = terminal(state.update(Message::ToggleFavoriteOutbound(
        OutboundId::new(scope.identity.engine.clone(), "node-b").unwrap(),
    )));
    assert_eq!(state.update(completion).units(), 0);
    assert!(
        state.runtime.proxy_groups[0]
            .proxies
            .iter()
            .find(|member| member.name == "node-b")
            .is_some_and(|member| member.favorite)
    );

    handler
        .2
        .lock()
        .unwrap()
        .push_back(CommandOutput::OutboundUiPreferencesApplied(
            outbound_preferences(
                &scope,
                44,
                "Node B",
                &["node-b"],
                OutboundHealthFilter::All,
                OutboundSortOrder::NameDescending,
                OutboundLayout::Grid,
            ),
        ));
    let completion = terminal(state.update(Message::SetOutboundSortOrder(
        OutboundSortOrder::NameDescending,
    )));
    assert_eq!(state.update(completion).units(), 0);
    assert_eq!(
        state.runtime.outbound_sort_order,
        OutboundSortOrder::NameDescending
    );
    assert_eq!(state.runtime.proxy_groups[0].proxies[0].name, "node-b");

    handler
        .2
        .lock()
        .unwrap()
        .push_back(CommandOutput::OutboundUiPreferencesApplied(
            outbound_preferences(
                &scope,
                45,
                "Node B",
                &["node-b"],
                OutboundHealthFilter::HealthyOnly,
                OutboundSortOrder::NameDescending,
                OutboundLayout::Grid,
            ),
        ));
    let completion = terminal(state.update(Message::ToggleFilterAlive(true)));
    assert_eq!(state.update(completion).units(), 0);
    assert!(state.runtime.proxy_groups[0].proxies.is_empty());
    assert!(state.runtime.filter_alive_only);

    handler
        .2
        .lock()
        .unwrap()
        .push_back(CommandOutput::OutboundUiPreferencesApplied(
            outbound_preferences(
                &scope,
                46,
                "Node B",
                &["node-b"],
                OutboundHealthFilter::HealthyOnly,
                OutboundSortOrder::NameDescending,
                OutboundLayout::CompactList,
            ),
        ));
    let completion = terminal(state.update(Message::ToggleProxyCompactView));
    assert_eq!(state.update(completion).units(), 0);
    assert!(state.runtime.proxy_compact_view);
    assert_eq!(
        state
            .runtime
            .outbound_inventory
            .data
            .as_ref()
            .unwrap()
            .scope(),
        &scope
    );
}

#[test]
fn product_search_keeps_failed_draft_and_retries_through_typed_phases() {
    let (mut state, _, scope, handler) = product_state(
        |scope| PageData::ready(inventory(scope, 61, SelectionAvailability::Selectable)),
        [],
    );
    let failure = Failure::new(ErrorCode::Permission, "search preference denied", false);
    *handler.1.lock().unwrap() = Some(failure.clone());

    let task = state.update(Message::FilterProxies("Node B".into()));
    assert!(matches!(
        &state.runtime.proxy_search.phase,
        ProxySearchPhase::Pending { query, .. } if query == "Node B"
    ));
    assert_eq!(state.update(terminal(task)).units(), 0);
    assert!(matches!(
        &state.runtime.proxy_search.phase,
        ProxySearchPhase::Failed { query, failure: actual }
            if query == "Node B" && actual == &failure
    ));
    assert_eq!(state.runtime.proxy_filter, "Node B");
    assert!(matches!(
        &state.runtime.proxy_preferences.phase,
        ProxyPreferencePhase::Failed(actual) if actual == &failure
    ));

    *handler.1.lock().unwrap() = None;
    handler
        .2
        .lock()
        .unwrap()
        .push_back(CommandOutput::OutboundUiPreferencesApplied(
            outbound_preferences(
                &scope,
                42,
                "Node B",
                &[],
                OutboundHealthFilter::All,
                OutboundSortOrder::LatencyAscending,
                OutboundLayout::Grid,
            ),
        ));
    let retry = state.update(Message::FilterProxies("Node B".into()));
    assert!(matches!(
        &state.runtime.proxy_search.phase,
        ProxySearchPhase::Pending { query, .. } if query == "Node B"
    ));
    assert_eq!(state.update(terminal(retry)).units(), 0);
    assert!(matches!(
        &state.runtime.proxy_search.phase,
        ProxySearchPhase::Synchronized { query } if query == "Node B"
    ));
}

#[test]
fn product_group_order_applies_only_after_typed_outbound_preference_readback() {
    let (mut state, _, scope, handler) = product_state(
        |scope| PageData::ready(inventory_with_two_collections(scope, 71)),
        [],
    );
    let original = state
        .runtime
        .proxy_groups
        .iter()
        .map(|group| group.name.clone())
        .collect::<Vec<_>>();
    assert_eq!(original, ["PROXY", "SECOND"]);
    let second_group = OutboundId::new(scope.identity.engine.clone(), "SECOND").unwrap();
    let proxy_group = OutboundId::new(scope.identity.engine.clone(), "PROXY").unwrap();
    let baseline = vec![proxy_group.clone(), second_group.clone()];

    let _ = state.update(Message::MoveProxyGroupUp(second_group.clone()));
    assert_eq!(
        state.runtime.group_order_editor.draft,
        vec![second_group.clone(), proxy_group.clone()]
    );
    assert_eq!(
        state.runtime.group_order_dialog,
        crate::state::runtime::ProxyGroupOrderDialogState::Editing
    );
    assert_eq!(
        state
            .runtime
            .proxy_groups
            .iter()
            .map(|group| group.name.as_str())
            .collect::<Vec<_>>(),
        original,
        "draft changes never reorder the accepted inventory"
    );

    let _ = state.update(Message::ResetProxyGroupOrder);
    assert_eq!(state.runtime.group_order_editor.draft, baseline);
    let _ = state.update(Message::MoveProxyGroupUp(second_group.clone()));
    let _ = state.update(Message::CancelProxyGroupOrder);
    assert_eq!(
        state.runtime.group_order_dialog,
        crate::state::runtime::ProxyGroupOrderDialogState::Closed
    );
    assert_eq!(state.runtime.group_order_editor.draft, baseline);

    let _ = state.update(Message::MoveProxyGroupUp(second_group.clone()));
    assert_eq!(
        state.runtime.group_order_editor.draft,
        vec![second_group.clone(), proxy_group.clone()]
    );

    let ordered = OutboundUiPreferencesSnapshot::new(
        scope.clone(),
        SnapshotRevision::new(42),
        OutboundUiPreferencesValues {
            search_query: String::new(),
            favorite_outbound_ids: Vec::new(),
            group_order: vec![second_group.clone(), proxy_group.clone()],
            collapsed_group_ids: Vec::new(),
            sort_order: OutboundSortOrder::LatencyAscending,
            health_filter: OutboundHealthFilter::All,
            layout: OutboundLayout::Grid,
        },
    )
    .unwrap();
    handler
        .2
        .lock()
        .unwrap()
        .push_back(CommandOutput::OutboundUiPreferencesApplied(ordered));
    let task = state.update(Message::ApplyProxyGroupOrder);
    assert!(matches!(
        &state.runtime.proxy_preferences.phase,
        ProxyPreferencePhase::Applying(_)
    ));
    let completion = terminal(task);
    assert_eq!(state.update(completion).units(), 0);
    assert_eq!(
        state.runtime.group_order_dialog,
        crate::state::runtime::ProxyGroupOrderDialogState::Closed
    );
    assert_eq!(
        state
            .runtime
            .proxy_groups
            .iter()
            .map(|group| group.name.as_str())
            .collect::<Vec<_>>(),
        ["SECOND", "PROXY"]
    );
    let inventory_revision = state
        .runtime
        .outbound_inventory
        .data
        .as_ref()
        .expect("accepted typed inventory")
        .revision();
    let expected_order = OutboundUiPreferenceIntent::ReorderGroups {
        group_ids: vec![second_group, proxy_group],
        inventory_revision,
    };
    assert!(matches!(
        handler.0.lock().unwrap().as_slice(),
        [CommandIntent::ApplyOutboundUiPreference { request }]
            if request.intent() == &expected_order
    ));
}

#[test]
fn product_selection_dispatches_read_outbound_intent_and_keeps_failed_state() {
    let failure = Failure::new(ErrorCode::Network, "outbound read failed", true);
    let (mut state, _, scope, handler) = product_state(
        |_| {
            PageData::unavailable(Failure::new(
                ErrorCode::NotReady,
                "waiting for inventory",
                true,
            ))
        },
        [CoreOperation::Outbound(OutboundOperation::List)],
    );
    *handler.1.lock().unwrap() = Some(failure.clone());

    let terminal = terminal(state.update(Message::RefreshOutboundInventory));
    let Message::OutboundInventoryReadFinished {
        requested_scope,
        terminal_scope,
        result,
    } = terminal
    else {
        panic!("RefreshOutboundInventory must read the typed outbound inventory")
    };
    assert_eq!(requested_scope, Some(scope.clone()));
    assert_eq!(terminal_scope, Some(scope));
    assert_eq!(result, Err(failure.clone()));
    assert_eq!(
        handler.0.lock().unwrap().as_slice(),
        [CommandIntent::ReadOutboundInventory]
    );

    assert_eq!(
        state
            .update(Message::OutboundInventoryReadFinished {
                requested_scope,
                terminal_scope,
                result,
            })
            .units(),
        0
    );
    assert_eq!(
        state.runtime.outbound_inventory.status,
        PageStatus::Failed { failure }
    );
    assert!(state.runtime.proxy_groups.is_empty());
}

#[test]
fn product_ingress_fails_closed_without_an_accepted_surface_scope() {
    let core = identified_core_application();
    install_test_core_capabilities(&core, [CoreOperation::Outbound(OutboundOperation::Select)]);
    let handler = Arc::new(RecordingHandler::default());
    core.install_command_handler(handler.clone());
    let scope = core.snapshot().scope().expect("identified core scope");
    let snapshot = inventory(&scope, 7, SelectionAvailability::Selectable);
    let mut state = AppState::empty();
    // Product commands stay unavailable until the app accepts a surface
    // snapshot; this fixture intentionally does not publish one.
    state.attach_product_application(core);
    state.apply_outbound_inventory_observation(PageData::ready(snapshot));

    assert!(
        state
            .outbound_selection_message("PROXY", "node-b")
            .is_none()
    );
    let request = OutboundSelectionRequest::new(
        OutboundId::new(scope.identity.engine.clone(), "PROXY").unwrap(),
        OutboundId::new(scope.identity.engine.clone(), "node-b").unwrap(),
        SnapshotRevision::new(7),
    )
    .unwrap();
    assert_eq!(
        state
            .update(Message::SelectOutbound { scope, request })
            .units(),
        0
    );
    assert!(handler.0.lock().unwrap().is_empty());
}
