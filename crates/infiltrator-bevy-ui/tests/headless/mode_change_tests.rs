//! test-intent: behavior
use crate::native_input::click_entity;
use crate::support::headless_plugins;
use bevy::a11y::AccessibilityNode;
use bevy::app::App;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::With;
use bevy::ui::{Display, Node};
use bevy::ui_widgets::ButtonPlugin;
use infiltrator_bevy_ui::app::{ModeActionState, PendingModeAck, ShellPlugin};
use infiltrator_bevy_ui::command_palette::{
    CommandPaletteRow, CommandPaletteState, OpenCommandPalette,
};
use infiltrator_bevy_ui::pages::overview::OverviewModePill;
use infiltrator_bevy_ui::pages::overview_cards::OverviewModeSegmentPill;
use infiltrator_bevy_ui::projection::{DemoOverviewSource, OverviewProjection, OverviewSource};
use infiltrator_bevy_ui::route::{ActiveRoute, PagesPlugin, Route};
use infiltrator_bevy_ui::shell_mode_issue::{
    DismissModeIssue, ModeIssueRoot, ModeIssueText, ModeSettingsGuide, RetryModeChange,
};
use infiltrator_bevy_ui::surface::{
    LatestSurfaceSnapshot, LegacyOverviewSurfaceSource, SurfaceSnapshotUpdated,
};
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_contract::command::ProxyMode;
use infiltrator_contract::command_catalogue::CommandTarget;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::snapshot::CoreLifecycle;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct DeferredSource {
    requests: Mutex<Vec<ModeRequest>>,
}
struct ModeRequest {
    target: ProxyMode,
    receipt: Sender<Result<ProxyMode, Failure>>,
}
impl OverviewSource for DeferredSource {
    fn current(&self) -> OverviewProjection {
        DemoOverviewSource::running().current()
    }
    fn set_mode(&self, target: ProxyMode, ack: Sender<Result<ProxyMode, Failure>>) {
        self.requests.lock().unwrap().push(ModeRequest {
            target,
            receipt: ack,
        });
    }
}
fn pill(app: &mut App, mode: ProxyMode) -> Entity {
    app.world_mut()
        .query::<(Entity, &OverviewModePill)>()
        .iter(app.world())
        .find(|(_, part)| part.0 == mode)
        .unwrap()
        .0
}
fn segment(app: &mut App, mode: ProxyMode) -> Entity {
    app.world_mut()
        .query::<(Entity, &OverviewModeSegmentPill)>()
        .iter(app.world())
        .find(|(_, pill)| pill.0 == mode)
        .unwrap()
        .0
}
fn entity<T: Component>(app: &mut App) -> Entity {
    app.world_mut()
        .query_filtered::<Entity, With<T>>()
        .single(app.world())
        .unwrap()
}

#[test]
fn native_mode_failure_retry_and_dismiss_keep_lifecycle_true_and_old_generation_receipts_cannot_finish_new_requests()
 {
    let source = Arc::new(DeferredSource::default());
    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins((
        ButtonPlugin,
        ShellPlugin::default(),
        PagesPlugin::new_surface(LegacyOverviewSurfaceSource::from_arc(source.clone())),
    ));
    app.update();
    let global = segment(&mut app, ProxyMode::Global);
    let direct = pill(&mut app, ProxyMode::Direct);
    let retry = entity::<RetryModeChange>(&mut app);
    let dismiss = entity::<DismissModeIssue>(&mut app);
    let root = entity::<ModeIssueRoot>(&mut app);
    click_entity(&mut app, global);
    let first = app.world().resource::<ModeActionState>().0.pending.unwrap();
    assert_eq!(source.requests.lock().unwrap().len(), 1);
    assert_eq!(source.requests.lock().unwrap()[0].target, ProxyMode::Global);
    assert!(app.world().get::<ButtonDisabled>(direct).unwrap().0);
    click_entity(&mut app, direct);
    assert_eq!(source.requests.lock().unwrap().len(), 1);
    let failure = Failure::new(ErrorCode::Network, "write refused temporarily", true);
    source.requests.lock().unwrap()[0]
        .receipt
        .send(Err(failure.clone()))
        .unwrap();
    app.update();
    assert_eq!(
        app.world().resource::<ModeActionState>().0.failure,
        Some(failure)
    );
    assert_eq!(
        app.world()
            .resource::<LatestSurfaceSnapshot>()
            .0
            .core
            .lifecycle,
        CoreLifecycle::Running
    );
    assert_eq!(
        app.world().get::<Node>(root).unwrap().display,
        Display::Flex
    );
    assert!(!app.world().get::<ButtonDisabled>(retry).unwrap().0);
    let issue_text = entity::<ModeIssueText>(&mut app);
    assert_eq!(
        app.world()
            .get::<AccessibilityNode>(issue_text)
            .unwrap()
            .0
            .role(),
        accesskit::Role::Alert
    );
    app.world_mut().insert_resource(UiLocale::new("en-US"));
    app.update();
    assert_eq!(entity::<ModeIssueText>(&mut app), issue_text);
    assert!(
        app.world()
            .get::<AccessibilityNode>(issue_text)
            .unwrap()
            .0
            .label()
            .unwrap()
            .contains("Global")
    );
    click_entity(&mut app, retry);
    let second = app.world().resource::<ModeActionState>().0.pending.unwrap();
    assert_ne!(second.token, first.token);
    assert_eq!(source.requests.lock().unwrap().len(), 2);
    assert_eq!(
        app.world().get::<Node>(root).unwrap().display,
        Display::None
    );
    let mut snapshot = app.world().resource::<LatestSurfaceSnapshot>().0.clone();
    snapshot.generation += 1;
    snapshot.core.generation = snapshot.generation;
    snapshot.revision = 1;
    app.world_mut().trigger(SurfaceSnapshotUpdated(snapshot));
    app.update();
    assert!(
        app.world()
            .resource::<ModeActionState>()
            .0
            .pending
            .is_none()
    );
    click_entity(&mut app, direct);
    let newest = app.world().resource::<ModeActionState>().0.pending.unwrap();
    assert_eq!(source.requests.lock().unwrap().len(), 3);
    assert!(
        source.requests.lock().unwrap()[1]
            .receipt
            .send(Ok(ProxyMode::Global))
            .is_err(),
        "retired receipt channel is closed"
    );
    assert_eq!(
        app.world().resource::<ModeActionState>().0.pending,
        Some(newest)
    );
    source.requests.lock().unwrap()[2]
        .receipt
        .send(Ok(ProxyMode::Direct))
        .unwrap();
    app.update();
    app.update();
    assert_eq!(
        app.world().resource::<ModeActionState>().0.observed.current,
        Some(ProxyMode::Direct)
    );
    assert!(app.world().resource::<PendingModeAck>().0.is_none());
    assert_eq!(
        app.world()
            .resource::<LatestSurfaceSnapshot>()
            .0
            .core
            .lifecycle,
        CoreLifecycle::Running
    );
    click_entity(&mut app, global);
    let denied = Failure::new(ErrorCode::Authentication, "authentication required", false);
    source.requests.lock().unwrap()[3]
        .receipt
        .send(Err(denied.clone()))
        .unwrap();
    app.update();
    assert!(app.world().get::<ButtonDisabled>(retry).unwrap().0);
    click_entity(&mut app, retry);
    assert_eq!(source.requests.lock().unwrap().len(), 4);
    click_entity(&mut app, dismiss);
    assert!(
        app.world()
            .resource::<ModeActionState>()
            .0
            .failure
            .is_none()
    );
    assert_eq!(
        app.world().get::<Node>(root).unwrap().display,
        Display::None
    );
    assert_eq!(
        app.world().resource::<ModeActionState>().0.observed.current,
        Some(ProxyMode::Direct)
    );
    assert_eq!(entity::<ModeIssueRoot>(&mut app), root);
    click_entity(&mut app, global);
    source.requests.lock().unwrap()[4]
        .receipt
        .send(Err(Failure::unsupported("mode control unavailable")))
        .unwrap();
    app.update();
    assert_eq!(
        app.world()
            .resource::<ModeActionState>()
            .0
            .failure
            .as_ref()
            .unwrap()
            .code,
        ErrorCode::Unsupported
    );
    assert!(app.world().get::<ButtonDisabled>(retry).unwrap().0);
    click_entity(&mut app, retry);
    assert_eq!(source.requests.lock().unwrap().len(), 5);
    click_entity(&mut app, dismiss);
    assert!(
        app.world()
            .resource::<ModeActionState>()
            .0
            .failure
            .is_none()
    );
    let index = {
        let catalogue = &app.world().resource::<CommandPaletteState>().catalogue;
        (0..catalogue.len())
            .find(|index| {
                catalogue.entry(*index).unwrap().target
                    == CommandTarget::SetProxyMode(ProxyMode::Global)
            })
            .unwrap()
    };
    app.world_mut().trigger(OpenCommandPalette);
    app.update();
    let row = app
        .world_mut()
        .query::<(Entity, &CommandPaletteRow)>()
        .iter(app.world())
        .find(|(_, row)| row.0 == index)
        .unwrap()
        .0;
    click_entity(&mut app, row);
    assert_eq!(source.requests.lock().unwrap().len(), 6);
    assert_eq!(
        app.world()
            .resource::<ModeActionState>()
            .0
            .pending
            .unwrap()
            .target,
        ProxyMode::Global
    );
    source.requests.lock().unwrap()[5]
        .receipt
        .send(Err(Failure::new(
            ErrorCode::Permission,
            "controller policy denies mode change",
            false,
        )))
        .unwrap();
    app.update();
    assert_eq!(
        app.world()
            .resource::<ModeActionState>()
            .0
            .failure
            .as_ref()
            .unwrap()
            .code,
        ErrorCode::Permission
    );
    assert_eq!(
        app.world().resource::<ModeActionState>().0.observed.current,
        Some(ProxyMode::Direct)
    );
    let guide = entity::<ModeSettingsGuide>(&mut app);
    assert_eq!(
        app.world().get::<Node>(guide).unwrap().display,
        Display::Flex
    );
    click_entity(&mut app, guide);
    assert_eq!(
        app.world().resource::<ActiveRoute>().0,
        Some(Route::Settings)
    );
    assert_eq!(source.requests.lock().unwrap().len(), 6);
    assert_eq!(
        app.world()
            .resource::<ModeActionState>()
            .0
            .failure
            .as_ref()
            .unwrap()
            .code,
        ErrorCode::Permission
    );
}
