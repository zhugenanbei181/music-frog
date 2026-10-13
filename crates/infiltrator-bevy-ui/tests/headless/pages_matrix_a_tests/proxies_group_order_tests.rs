use super::*;
use infiltrator_bevy_ui::pages::proxy_group_order::GroupOrderDialogState;
use infiltrator_bevy_ui::pages::proxy_inspection::{ProxyInspectionPhase, ProxyInspectionState};

#[test]
fn test_proxies_node_detail_drawer_and_group_reorder() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink.clone());
    let _ = navigate_to(&mut app, Route::Proxies);

    // 1. Detail button activation (DUAL-04-11)
    let detail_entity = {
        let world = app.world_mut();
        let mut query = world.query::<(Entity, &NodeDetailButton)>();
        query
            .iter(world)
            .next()
            .expect("node detail button mounted")
            .0
    };

    app.world_mut().commands().trigger(Activate {
        entity: detail_entity,
    });
    app.update();

    assert!(
        sink.submitted().is_empty(),
        "inspection must never select a node"
    );
    assert!(matches!(
        &app.world().resource::<ProxyInspectionState>().phase,
        ProxyInspectionPhase::Open(_)
    ));

    // 2. Group move up activation (DUAL-04-12)
    let move_up_entity = {
        let world = app.world_mut();
        let mut query = world.query::<(Entity, &ProxyGroupMoveUpButton)>();
        query
            .iter(world)
            .nth(1)
            .expect("second group move up button mounted")
            .0
    };

    app.world_mut().commands().trigger(Activate {
        entity: move_up_entity,
    });
    app.update();

    assert!(
        sink.submitted().is_empty(),
        "order editing must not submit any command before Apply"
    );
    assert_eq!(
        app.world().resource::<GroupOrderState>().dialog,
        GroupOrderDialogState::Editing
    );
    assert_ne!(
        app.world().resource::<GroupOrderState>().editor.draft,
        app.world().resource::<GroupOrderState>().editor.baseline
    );

    // 3. Reset group order activation (DUAL-04-12)
    let reset_entity = app
        .world_mut()
        .query_filtered::<Entity, With<ResetProxyGroupOrderButton>>()
        .single(app.world())
        .expect("reset proxy group order button mounted");

    app.world_mut().commands().trigger(Activate {
        entity: reset_entity,
    });
    app.update();

    assert!(
        sink.submitted().is_empty(),
        "reset changes only the visible draft"
    );
    let cancel = app
        .world_mut()
        .query::<(Entity, &GroupOrderAction)>()
        .iter(app.world())
        .find(|(_, action)| matches!(action, GroupOrderAction::Cancel))
        .unwrap()
        .0;
    app.world_mut()
        .commands()
        .trigger(Activate { entity: cancel });
    app.update();
    assert_eq!(
        app.world().resource::<GroupOrderState>().dialog,
        GroupOrderDialogState::Closed
    );
    assert!(
        sink.submitted().is_empty(),
        "cancel retains the original shared order"
    );

    // 4. Toggle compact view activation (DUAL-04-13)
    let toggle_view_entity = app
        .world_mut()
        .query_filtered::<Entity, With<ToggleViewModeButton>>()
        .single(app.world())
        .expect("toggle view mode button mounted");

    app.world_mut().commands().trigger(Activate {
        entity: toggle_view_entity,
    });
    app.update();

    assert!(
        sink.submitted()
            .iter()
            .any(|cmd| matches!(
                cmd,
                UiCommand::ApplyOutboundUiPreference { request }
                    if request.intent() == &OutboundUiPreferenceIntent::SetLayout(OutboundLayout::CompactList)
            ))
    );

    // 5. Latency skeleton pulse mounted (DUAL-04-14)
    assert!(
        app.world_mut()
            .query::<&LatencySkeletonPulse>()
            .iter(app.world())
            .next()
            .is_some(),
        "latency skeleton pulse component must be mounted"
    );
}
