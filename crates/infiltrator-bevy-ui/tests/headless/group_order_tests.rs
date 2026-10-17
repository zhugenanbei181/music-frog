//! test-intent: behavior
//! Group order edits are revision-fenced and carry only opaque collection IDs.
use crate::command_harness::TypedOutboundPreferenceSink;
use crate::support::headless_plugins;
use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ui_widgets::Activate;
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::command::CommandPumpPlugin;
use infiltrator_bevy_ui::command::UiCommand;
use infiltrator_bevy_ui::pages::proxies::{ProxyGroupMoveUpButton, ProxyNodeButton};
use infiltrator_bevy_ui::pages::proxy_group_order::{
    GroupOrderAction, GroupOrderDialogState, GroupOrderList, GroupOrderRow, GroupOrderState,
};
use infiltrator_bevy_ui::route::{PagesPlugin, Route, RouteChanged};
use infiltrator_bevy_ui::surface::{
    DemoSurfaceSource, LatestSurfaceSnapshot, SurfaceSnapshotUpdated, SurfaceSource,
};
use infiltrator_contract::surface_snapshot::PageData;
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_presentation::proxy_group_order_editor::GroupMove;
use lurker_core_contract::error::ErrorCode;
use lurker_core_contract::error::Failure;
use lurker_core_contract::outbound::identity::OutboundId;
use lurker_core_contract::outbound::inventory::OutboundInventoryState;
use lurker_core_contract::outbound::preferences::OutboundUiPreferenceIntent;
use std::sync::Arc;
use std::thread::yield_now;
use std::time::{Duration, Instant};

fn action(app: &mut App, predicate: impl Fn(&GroupOrderAction) -> bool) -> Entity {
    app.world_mut()
        .query::<(Entity, &GroupOrderAction)>()
        .iter(app.world())
        .find(|(_, action)| predicate(action))
        .unwrap()
        .0
}
fn activate(app: &mut App, entity: Entity) {
    app.world_mut().commands().trigger(Activate { entity });
    app.update();
    app.update();
}
fn settle(app: &mut App) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while app
        .world()
        .resource::<GroupOrderState>()
        .editor
        .is_pending()
    {
        assert!(Instant::now() < deadline, "typed order request must finish");
        app.update();
        yield_now();
    }
    app.update();
}
fn rows(app: &mut App) -> Vec<(Entity, String)> {
    let root = app
        .world_mut()
        .query::<(Entity, &GroupOrderList)>()
        .iter(app.world())
        .next()
        .unwrap()
        .0;
    app.world()
        .get::<Children>(root)
        .unwrap()
        .iter()
        .map(|entity| {
            (
                *entity,
                app.world()
                    .get::<GroupOrderRow>(*entity)
                    .unwrap()
                    .label
                    .clone(),
            )
        })
        .collect()
}
fn setup() -> (App, TypedOutboundPreferenceSink) {
    let mut snapshot = DemoSurfaceSource::running().surface_snapshot();
    let preferences = snapshot
        .outbound_ui_preferences
        .data
        .clone()
        .expect("demo has typed preferences");
    let sink = TypedOutboundPreferenceSink::new(preferences);
    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins(ShellPlugin::default());
    app.add_plugins(PagesPlugin::new_surface(DemoSurfaceSource::running()));
    app.add_plugins(CommandPumpPlugin::new(Arc::new(sink.clone())));
    app.update();
    app.world_mut()
        .commands()
        .trigger(RouteChanged(Route::Proxies));
    app.update();
    snapshot.revision += 1;
    publish(&mut app, &sink, snapshot);
    (app, sink)
}
fn publish(app: &mut App, sink: &TypedOutboundPreferenceSink, mut snapshot: SurfaceSnapshot) {
    snapshot.outbound_ui_preferences = PageData::ready(sink.preferences());
    publish_raw(app, snapshot);
}
fn publish_raw(app: &mut App, snapshot: SurfaceSnapshot) {
    let mut snapshot = snapshot;
    snapshot.revision = app
        .world()
        .resource::<LatestSurfaceSnapshot>()
        .0
        .revision
        .saturating_add(1);
    app.world_mut()
        .commands()
        .trigger(SurfaceSnapshotUpdated(snapshot));
    app.update();
}
fn ids(app: &App) -> Vec<OutboundId> {
    let latest = app.world().resource::<LatestSurfaceSnapshot>();
    match latest
        .0
        .outbound_inventory
        .data
        .as_ref()
        .unwrap()
        .inventory()
    {
        OutboundInventoryState::Ready { collections, .. } => collections
            .iter()
            .map(|collection| collection.id.clone())
            .collect(),
        _ => unreachable!("demo inventory is ready"),
    }
}

#[test]
fn native_order_editor_reorders_typed_ids_preserves_rows_and_applies_readback() {
    let (mut app, sink) = setup();
    let ids = ids(&app);
    let names: Vec<_> = ids.iter().map(|id| id.value().to_owned()).collect();
    let nodes: Vec<_> = app
        .world_mut()
        .query::<(Entity, &ProxyNodeButton)>()
        .iter(app.world())
        .map(|(entity, _)| entity)
        .collect();
    let up = app
        .world_mut()
        .query::<(Entity, &ProxyGroupMoveUpButton)>()
        .iter(app.world())
        .find(|(_, button)| button.group_name == names[1])
        .unwrap()
        .0;
    activate(&mut app, up);
    assert_eq!(
        app.world().resource::<GroupOrderState>().dialog,
        GroupOrderDialogState::Editing
    );
    let moved = rows(&mut app);
    assert_eq!(moved[0].1, names[1]);
    assert!(sink.preferences().group_order.is_empty());
    let down = action(
        &mut app,
        |action| matches!(action, GroupOrderAction::Move { group_id, direction: GroupMove::Down } if group_id.value() == names[1]),
    );
    activate(&mut app, down);
    let restored = rows(&mut app);
    assert_eq!(
        restored
            .iter()
            .map(|(_, label)| label.clone())
            .collect::<Vec<_>>(),
        names
    );
    assert!(restored.iter().all(|row| moved.contains(row)));
    let cancel = action(&mut app, |action| {
        matches!(action, GroupOrderAction::Cancel)
    });
    activate(&mut app, cancel);
    assert!(sink.preferences().group_order.is_empty());

    activate(&mut app, up);
    let expected = app
        .world()
        .resource::<GroupOrderState>()
        .editor
        .draft()
        .to_vec();
    let mut failed_read = app.world().resource::<LatestSurfaceSnapshot>().0.clone();
    failed_read.outbound_ui_preferences = PageData::failed(Failure::new(
        ErrorCode::Network,
        "preferences read failed",
        true,
    ));
    publish_raw(&mut app, failed_read);
    assert!(!app.world().resource::<GroupOrderState>().editor.can_apply());
    let apply = action(&mut app, |action| matches!(action, GroupOrderAction::Apply));
    activate(&mut app, apply);
    assert!(sink.preferences().group_order.is_empty());

    let ready = app.world().resource::<LatestSurfaceSnapshot>().0.clone();
    publish(&mut app, &sink, ready);
    assert_eq!(
        app.world().resource::<GroupOrderState>().editor.draft(),
        expected
    );
    activate(&mut app, apply);
    settle(&mut app);
    assert_eq!(
        app.world().resource::<GroupOrderState>().dialog,
        GroupOrderDialogState::Closed
    );
    assert_eq!(sink.preferences().group_order, expected);
    let published = app.world().resource::<LatestSurfaceSnapshot>().0.clone();
    publish(&mut app, &sink, published);
    assert_eq!(
        app.world()
            .resource::<LatestSurfaceSnapshot>()
            .0
            .outbound_ui_preferences
            .data
            .as_ref()
            .unwrap()
            .group_order,
        expected
    );
    assert!(
        nodes
            .iter()
            .all(|entity| app.world().get::<ProxyNodeButton>(*entity).is_some())
    );
    let command = sink.commands().last().unwrap().clone();
    let UiCommand::ApplyOutboundUiPreference { request } = command else {
        panic!("group order submits typed preference command")
    };
    assert_eq!(request.expected_revision().value(), 1);
    let inventory_revision = app
        .world()
        .resource::<LatestSurfaceSnapshot>()
        .0
        .outbound_inventory
        .data
        .as_ref()
        .expect("typed outbound inventory")
        .revision();
    assert_eq!(
        request.intent(),
        &OutboundUiPreferenceIntent::ReorderGroups {
            group_ids: expected,
            inventory_revision,
        }
    );
}

#[test]
fn rejected_native_order_apply_keeps_typed_draft_and_retry_sends_the_same_revision_fenced_intent() {
    let (mut app, sink) = setup();
    let up = app
        .world_mut()
        .query::<(Entity, &ProxyGroupMoveUpButton)>()
        .iter(app.world())
        .nth(1)
        .unwrap()
        .0;
    activate(&mut app, up);
    let expected = app
        .world()
        .resource::<GroupOrderState>()
        .editor
        .draft()
        .to_vec();
    let failure = Failure::new(
        ErrorCode::Storage,
        "typed preferences refused the order",
        true,
    );
    sink.fail_next(failure.clone());
    let apply = action(&mut app, |action| matches!(action, GroupOrderAction::Apply));
    activate(&mut app, apply);
    settle(&mut app);
    let state = app.world().resource::<GroupOrderState>();
    assert_eq!(state.dialog, GroupOrderDialogState::Editing);
    assert_eq!(state.editor.failure(), Some(&failure));
    assert_eq!(state.editor.draft(), expected);
    assert!(sink.preferences().group_order.is_empty());
    activate(&mut app, apply);
    settle(&mut app);
    assert_eq!(
        app.world().resource::<GroupOrderState>().dialog,
        GroupOrderDialogState::Closed
    );
    assert_eq!(sink.preferences().group_order, expected);
}
