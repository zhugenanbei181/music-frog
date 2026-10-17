use super::*;

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
        state.runtime.group_order_editor.draft(),
        vec![second_group.clone(), proxy_group.clone()]
    );
    assert_eq!(
        state.runtime.group_order_dialog,
        ProxyGroupOrderDialogState::Editing
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
    assert_eq!(state.runtime.group_order_editor.draft(), baseline);
    let _ = state.update(Message::MoveProxyGroupUp(second_group.clone()));
    let _ = state.update(Message::CancelProxyGroupOrder);
    assert_eq!(state.runtime.group_order_dialog, Closed);
    assert_eq!(state.runtime.group_order_editor.draft(), baseline);

    let _ = state.update(Message::MoveProxyGroupUp(second_group.clone()));
    assert_eq!(
        state.runtime.group_order_editor.draft(),
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
    assert_eq!(state.runtime.group_order_dialog, Closed);
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
