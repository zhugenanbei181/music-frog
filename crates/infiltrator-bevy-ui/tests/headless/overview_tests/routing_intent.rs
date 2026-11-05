//! Behavior cases for shared routing intent controls.
//! test-intent: behavior

use super::*;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ui::prelude::{Display, FlexWrap, Node};
use infiltrator_bevy_ui::app::RoutingIntentActionState;
use infiltrator_bevy_ui::pages::overview_cards::{
    OverviewRoutingIntentSegmentPill, RoutingIntentSegmentCard,
};
use lurker_core_contract::routing_intent::RoutingIntent;

#[test]
fn overview_routing_intent_submits_the_shared_typed_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.init_asset::<Image>();
    app.add_plugins(ShellPlugin::default());
    app.add_plugins(PagesPlugin::new(DemoOverviewSource::running()));
    app.add_plugins(CommandPumpPlugin::new(sink.clone()));
    app.update();

    let button = {
        let world = app.world_mut();
        let mut buttons = world.query::<(Entity, &OverviewRoutingIntentSegmentPill)>();
        buttons
            .iter(world)
            .find(|(_, pill)| pill.0 == RoutingIntent::RouteAllThroughProxy)
            .expect("route all through proxy pill mounted")
            .0
    };
    app.world_mut()
        .commands()
        .trigger(Activate { entity: button });
    app.update();
    app.update();

    let actions = &app.world().resource::<RoutingIntentActionState>().0;
    assert!(
        actions.pending_change().is_none(),
        "demo command feedback completes the typed transition"
    );
    assert_eq!(
        actions.render_snapshot().current,
        Some(RoutingIntent::RouteAllThroughProxy)
    );
    assert_eq!(
        sink.submitted(),
        vec![UiCommand::SetRoutingIntent(
            RoutingIntent::RouteAllThroughProxy
        )]
    );
}

#[test]
fn overview_routing_intent_card_exposes_only_the_three_shared_choices() {
    let mut app = mounted_default();
    let world = app.world_mut();
    let mut card_query = world.query::<&RoutingIntentSegmentCard>();
    assert!(
        card_query.iter(world).next().is_some(),
        "RoutingIntentSegmentCard must be mounted"
    );

    let mut pills_query = world.query::<&OverviewRoutingIntentSegmentPill>();
    let intents: Vec<RoutingIntent> = pills_query.iter(world).map(|pill| pill.0.clone()).collect();
    assert_eq!(
        intents,
        vec![
            RoutingIntent::FollowRules,
            RoutingIntent::RouteAllThroughProxy,
            RoutingIntent::BypassProxy,
        ]
    );
}

#[test]
fn routing_intent_choices_share_one_three_column_row() {
    let mut app = mounted_default();
    let world = app.world_mut();

    // Read the row through the choices' own parent: that pins this card's
    // geometry instead of matching some other grid on the page.
    {
        let mut pills = world.query::<&OverviewRoutingIntentSegmentPill>();
        assert_eq!(pills.iter(world).count(), 3, "three routing choices mount");
    }
    let row_entity = {
        let mut parents = world.query::<(&OverviewRoutingIntentSegmentPill, &ChildOf)>();
        let shared = parents
            .iter(world)
            .map(|(_, parent_of)| parent_of.parent())
            .reduce(|same, parent| {
                assert_eq!(same, parent, "all three choices share one row");
                same
            });
        shared.expect("three routing choices mount")
    };

    let row = world.get::<Node>(row_entity).expect("the shared row");
    assert_eq!(row.display, Display::Grid, "the row is a grid");
    assert_eq!(
        row.grid_template_columns.len(),
        3,
        "the row owns exactly three columns, one per choice"
    );
    assert_eq!(row.grid_template_rows.len(), 1, "one row owns all three");
    assert_eq!(
        row.flex_wrap,
        FlexWrap::NoWrap,
        "a wrapping row is what stacked the third choice under the other two"
    );
}
