//! test-intent: behavior
use crate::native_input::click_entity;
use crate::support::headless_plugins;
use bevy::app::App;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::With;
use bevy::ui::widget::Text;
use bevy::ui::{Display, Node};
use bevy::ui_widgets::ButtonPlugin;
use infiltrator_bevy_ui::app::{RoutingIntentActionState, ShellPlugin};
use infiltrator_bevy_ui::command::{CommandPumpPlugin, UiCommand, UiCommandSink};
use infiltrator_bevy_ui::command_events::CommandExecutedEvent;
use infiltrator_bevy_ui::command_palette::{
    CommandPaletteRow, CommandPaletteState, OpenCommandPalette,
};
use infiltrator_bevy_ui::pages::overview_cards::OverviewRoutingIntentSegmentPill;
use infiltrator_bevy_ui::route::{ActiveRoute, PagesPlugin, Route};
use infiltrator_bevy_ui::shell_routing_intent_issue::{
    DismissRoutingIntentIssue, RetryRoutingIntentChange, RoutingIntentIssueRoot,
    RoutingIntentIssueText, RoutingIntentSettingsGuide,
};
use infiltrator_bevy_ui::surface::{LatestSurfaceSnapshot, SurfaceSnapshotUpdated};
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_contract::command::RequestId;
use infiltrator_contract::command_catalogue::CommandTarget;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_presentation::routing_intent_projection::routing_intent_failure_copy;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::routing_intent::RoutingIntent;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct DeferredRoutingIntentSink {
    next_request: AtomicU64,
    submitted: Mutex<Vec<UiCommand>>,
}

impl DeferredRoutingIntentSink {
    fn submitted(&self) -> Vec<UiCommand> {
        self.submitted.lock().unwrap().clone()
    }
}

impl UiCommandSink for DeferredRoutingIntentSink {
    fn submit(&self, command: UiCommand) {
        self.submitted.lock().unwrap().push(command);
    }

    fn submit_tracked(&self, command: UiCommand) -> Option<RequestId> {
        self.submit(command);
        Some(RequestId(
            self.next_request.fetch_add(1, Ordering::Relaxed) + 1,
        ))
    }
}

fn pill(app: &mut App, intent: &RoutingIntent) -> Entity {
    app.world_mut()
        .query::<(Entity, &OverviewRoutingIntentSegmentPill)>()
        .iter(app.world())
        .find(|(_, pill)| &pill.0 == intent)
        .expect("routing choice mounted")
        .0
}

fn entity<T: Component>(app: &mut App) -> Entity {
    app.world_mut()
        .query_filtered::<Entity, With<T>>()
        .single(app.world())
        .unwrap()
}

fn app_with_sink(sink: Arc<DeferredRoutingIntentSink>) -> App {
    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins((
        ButtonPlugin,
        CommandPumpPlugin::new(sink.clone()),
        ShellPlugin::default(),
        PagesPlugin::demo(),
    ));
    app.update();
    app
}

fn result(app: &mut App, id: u64, intent: RoutingIntent, result: Result<CommandOutput, Failure>) {
    app.world_mut().trigger(CommandExecutedEvent {
        command: UiCommand::SetRoutingIntent(intent),
        request_id: RequestId(id),
        result,
    });
    app.update();
}

#[test]
fn routing_intent_command_failure_retry_authentication_and_palette_share_one_typed_path() {
    let sink = Arc::new(DeferredRoutingIntentSink::default());
    let mut app = app_with_sink(sink.clone());
    let route_all = pill(&mut app, &RoutingIntent::RouteAllThroughProxy);
    let retry = entity::<RetryRoutingIntentChange>(&mut app);
    let dismiss = entity::<DismissRoutingIntentIssue>(&mut app);
    let issue = entity::<RoutingIntentIssueRoot>(&mut app);

    click_entity(&mut app, route_all);
    let first = app
        .world()
        .resource::<RoutingIntentActionState>()
        .0
        .pending_change()
        .cloned()
        .unwrap();
    assert_eq!(first.target, RoutingIntent::RouteAllThroughProxy);
    assert_eq!(
        sink.submitted(),
        vec![UiCommand::SetRoutingIntent(first.target.clone())]
    );
    assert!(app.world().get::<ButtonDisabled>(retry).unwrap().0);

    let failure = Failure::new(ErrorCode::Network, "write refused temporarily", true);
    result(&mut app, 1, first.target.clone(), Err(failure.clone()));
    assert_eq!(
        app.world()
            .resource::<RoutingIntentActionState>()
            .0
            .failure(),
        Some(&failure)
    );
    assert_eq!(
        app.world().get::<Node>(issue).unwrap().display,
        Display::Flex
    );
    assert!(!app.world().get::<ButtonDisabled>(retry).unwrap().0);
    let issue_text = entity::<RoutingIntentIssueText>(&mut app);
    app.world_mut().insert_resource(UiLocale::new("en-US"));
    app.update();
    let expected_issue_copy = routing_intent_failure_copy(
        &app.world().resource::<RoutingIntentActionState>().0,
        "en-US",
    );
    assert_eq!(
        app.world().get::<Text>(issue_text).unwrap().0,
        expected_issue_copy
    );

    click_entity(&mut app, retry);
    let second = app
        .world()
        .resource::<RoutingIntentActionState>()
        .0
        .pending_change()
        .cloned()
        .unwrap();
    assert_ne!(second.token, first.token);
    result(
        &mut app,
        2,
        second.target.clone(),
        Ok(CommandOutput::RoutingIntentApplied {
            observed: RoutingIntent::RouteAllThroughProxy,
        }),
    );
    assert!(
        app.world()
            .resource::<RoutingIntentActionState>()
            .0
            .pending_change()
            .is_none()
    );
    assert_eq!(
        app.world()
            .resource::<RoutingIntentActionState>()
            .0
            .render_snapshot()
            .current,
        Some(RoutingIntent::RouteAllThroughProxy)
    );

    let bypass = pill(&mut app, &RoutingIntent::BypassProxy);
    click_entity(&mut app, bypass);
    let third = app
        .world()
        .resource::<RoutingIntentActionState>()
        .0
        .pending_change()
        .cloned()
        .unwrap();
    result(
        &mut app,
        3,
        third.target.clone(),
        Err(Failure::new(
            ErrorCode::Authentication,
            "authentication required",
            false,
        )),
    );
    assert!(app.world().get::<ButtonDisabled>(retry).unwrap().0);
    let guide = entity::<RoutingIntentSettingsGuide>(&mut app);
    assert_eq!(
        app.world().get::<Node>(guide).unwrap().display,
        Display::Flex
    );
    click_entity(&mut app, dismiss);
    assert!(
        app.world()
            .resource::<RoutingIntentActionState>()
            .0
            .failure()
            .is_none()
    );
    assert_eq!(
        app.world().get::<Node>(issue).unwrap().display,
        Display::None
    );

    let catalogue = &app.world().resource::<CommandPaletteState>().catalogue;
    let index = (0..catalogue.len())
        .find(|index| {
            catalogue.entry(*index).unwrap().target
                == CommandTarget::SetRoutingIntent(RoutingIntent::FollowRules)
        })
        .unwrap();
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
    let fourth = app
        .world()
        .resource::<RoutingIntentActionState>()
        .0
        .pending_change()
        .cloned()
        .unwrap();
    assert_eq!(fourth.target, RoutingIntent::FollowRules);
    assert_eq!(
        sink.submitted().last(),
        Some(&UiCommand::SetRoutingIntent(RoutingIntent::FollowRules))
    );

    let mut snapshot = app.world().resource::<LatestSurfaceSnapshot>().0.clone();
    snapshot.generation += 1;
    snapshot.core.generation = snapshot.generation;
    snapshot.revision = 1;
    app.world_mut().trigger(SurfaceSnapshotUpdated(snapshot));
    app.update();
    result(
        &mut app,
        4,
        fourth.target,
        Ok(CommandOutput::RoutingIntentApplied {
            observed: RoutingIntent::FollowRules,
        }),
    );
    assert!(
        app.world()
            .resource::<RoutingIntentActionState>()
            .0
            .pending_change()
            .is_none()
    );
    assert_eq!(
        app.world()
            .resource::<RoutingIntentActionState>()
            .0
            .render_snapshot()
            .current,
        Some(RoutingIntent::RouteAllThroughProxy),
        "a result from a retired generation cannot update the observed choice"
    );
    assert_eq!(
        app.world().resource::<ActiveRoute>().0,
        Some(Route::Overview)
    );
}
