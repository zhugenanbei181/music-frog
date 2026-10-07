//! test-intent: behavior
use crate::command_harness::{RecordingHandler, recording_application};
use crate::support::headless_plugins;
use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ui_widgets::Activate;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::proxy_group_order_editor::GroupMove;
use infiltrator_application::proxy_preferences_application::ProxyPreferencesApplication;
use infiltrator_application::proxy_projection::project_proxy_groups;
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::command::CommandPumpPlugin;
use infiltrator_bevy_ui::pages::proxies::{ProxyGroupMoveUpButton, ProxyNodeButton};
use infiltrator_bevy_ui::pages::proxy_group_order::{
    GroupOrderAction, GroupOrderList, GroupOrderRow, GroupOrderState,
};
use infiltrator_bevy_ui::route::{PagesPlugin, Route, RouteChanged};
use infiltrator_bevy_ui::surface::{
    DemoSurfaceSource, LatestSurfaceSnapshot, SurfaceSnapshotUpdated, SurfaceSource,
};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::surface_snapshot::PageData;
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
        .pending
        .is_some()
    {
        assert!(
            Instant::now() < deadline,
            "native order waits for actual terminal result"
        );
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
                app.world().get::<GroupOrderRow>(*entity).unwrap().0.clone(),
            )
        })
        .collect()
}
fn setup() -> (App, Arc<CoreApplication>, ProxyPreferencesApplication) {
    let preferences = ProxyPreferencesApplication::new();
    let (application, _) = recording_application();
    application.install_command_handler(Arc::new(
        CommandApplication::new().with_proxy_preferences(preferences.clone()),
    ));
    let application = Arc::new(application);
    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins(ShellPlugin::default());
    app.add_plugins(PagesPlugin::demo());
    app.add_plugins(CommandPumpPlugin::for_application(application.clone()));
    app.update();
    app.world_mut()
        .commands()
        .trigger(RouteChanged(Route::Proxies));
    app.update();
    (app, application, preferences)
}

#[test]
fn native_order_editor_preserves_row_entities_cancel_has_no_shared_effect_and_apply_executes_the_complete_order()
 {
    let (mut app, _, preferences) = setup();
    let mut snapshot = app.world().resource::<LatestSurfaceSnapshot>().0.clone();
    let original = snapshot.pages.proxies.data.as_ref().unwrap().groups.clone();
    let names: Vec<_> = original.iter().map(|group| group.name.clone()).collect();
    let selected: Vec<_> = original.iter().map(|group| group.current.clone()).collect();
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
    assert!(app.world().resource::<GroupOrderState>().open);
    let moved = rows(&mut app);
    assert_eq!(moved[0].1, names[1]);
    assert!(preferences.custom_group_order().unwrap().is_empty());
    let down = action(
        &mut app,
        |action| matches!(action, GroupOrderAction::Move { name, direction: GroupMove::Down } if name == &names[1]),
    );
    activate(&mut app, down);
    let restored = rows(&mut app);
    assert_eq!(
        restored
            .iter()
            .map(|(_, name)| name.clone())
            .collect::<Vec<_>>(),
        names
    );
    assert!(
        restored.iter().all(|row| moved.contains(row)),
        "moving changes child order without replacing surviving native controls"
    );
    let cancel = action(&mut app, |action| {
        matches!(action, GroupOrderAction::Cancel)
    });
    activate(&mut app, cancel);
    assert!(preferences.custom_group_order().unwrap().is_empty());
    activate(&mut app, up);
    let expected = app
        .world()
        .resource::<GroupOrderState>()
        .editor
        .draft
        .clone();
    snapshot.revision += 1;
    snapshot.pages.proxies =
        PageData::failed(Failure::new(ErrorCode::Network, "read failed", true));
    app.world_mut()
        .commands()
        .trigger(SurfaceSnapshotUpdated(snapshot.clone()));
    app.update();
    assert!(!app.world().resource::<GroupOrderState>().editor.can_apply());
    let apply = action(&mut app, |action| matches!(action, GroupOrderAction::Apply));
    activate(&mut app, apply);
    assert!(preferences.custom_group_order().unwrap().is_empty());
    snapshot.revision += 1;
    let mut ready = app.world().resource::<LatestSurfaceSnapshot>().0.clone();
    ready.pages.proxies = PageData::ready({
        let mut page = DemoSurfaceSource::running()
            .surface_snapshot()
            .pages
            .proxies
            .data
            .unwrap();
        page.groups = original.clone();
        page
    });
    ready.revision = snapshot.revision;
    app.world_mut()
        .commands()
        .trigger(SurfaceSnapshotUpdated(ready.clone()));
    app.update();
    assert_eq!(
        app.world().resource::<GroupOrderState>().editor.draft,
        expected
    );
    activate(&mut app, apply);
    settle(&mut app);
    assert!(!app.world().resource::<GroupOrderState>().open);
    assert_eq!(preferences.custom_group_order().unwrap(), expected);
    ready.revision += 1;
    ready.pages.proxies.data.as_mut().unwrap().groups =
        project_proxy_groups(original.clone(), &preferences.preferences().unwrap());
    app.world_mut()
        .commands()
        .trigger(SurfaceSnapshotUpdated(ready));
    app.update();
    let page = &app
        .world()
        .resource::<LatestSurfaceSnapshot>()
        .0
        .pages
        .proxies
        .data
        .as_ref()
        .unwrap();
    assert_eq!(
        page.groups
            .iter()
            .map(|group| group.name.clone())
            .collect::<Vec<_>>(),
        expected
    );
    for (index, group) in original.iter().enumerate() {
        assert_eq!(
            page.groups
                .iter()
                .find(|item| item.name == group.name)
                .unwrap()
                .current,
            selected[index]
        );
    }
    assert!(
        nodes
            .iter()
            .all(|entity| app.world().get::<ProxyNodeButton>(*entity).is_some())
    );
}

#[test]
fn rejected_native_apply_keeps_full_draft_and_retry_publishes_only_real_shared_command_success() {
    let (mut app, application, preferences) = setup();
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
        .draft
        .clone();
    let failure = Failure::new(ErrorCode::Storage, "shared state refused the order", true);
    let handler = Arc::new(RecordingHandler::default());
    *handler.1.lock().unwrap() = Some(failure.clone());
    application.install_command_handler(handler);
    let apply = action(&mut app, |action| matches!(action, GroupOrderAction::Apply));
    activate(&mut app, apply);
    settle(&mut app);
    let state = app.world().resource::<GroupOrderState>();
    assert!(state.open);
    assert_eq!(state.editor.failure, Some(failure));
    assert_eq!(state.editor.draft, expected);
    assert!(preferences.custom_group_order().unwrap().is_empty());
    application.install_command_handler(Arc::new(
        CommandApplication::new().with_proxy_preferences(preferences.clone()),
    ));
    activate(&mut app, apply);
    settle(&mut app);
    assert!(!app.world().resource::<GroupOrderState>().open);
    assert_eq!(preferences.custom_group_order().unwrap(), expected);
}
