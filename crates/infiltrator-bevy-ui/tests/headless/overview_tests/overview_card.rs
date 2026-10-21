//! Behavior cases for overview card.
//! test-intent: behavior

use super::*;
use infiltrator_contract::overview_layout::OverviewCardKind;

#[test]
fn overview_card_reorder_actions_submit_commands() {
    let sink = Arc::new(DemoCommandSink::accepting());
    sink.submit(UiCommand::MoveOverviewCardUp(OverviewCardKind::Traffic));
    sink.submit(UiCommand::MoveOverviewCardDown(OverviewCardKind::Metrics));
    sink.submit(UiCommand::ResetOverviewCardOrder);

    let items = sink.submitted();
    assert_eq!(items.len(), 3);
    assert_eq!(
        items[0],
        UiCommand::MoveOverviewCardUp(OverviewCardKind::Traffic)
    );
    assert_eq!(
        items[1],
        UiCommand::MoveOverviewCardDown(OverviewCardKind::Metrics)
    );
    assert_eq!(items[2], UiCommand::ResetOverviewCardOrder);
}

/// The Overview page mounts one reorderable slot per card, and a move
/// publishes the shared reorder intent with the *whole* new order.
#[test]
fn overview_page_mounts_reorderable_slots_and_publishes_the_shared_intent() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.init_asset::<Image>();
    app.add_plugins(ShellPlugin::default());
    app.add_plugins(PagesPlugin::demo());
    app.add_plugins(CommandPumpPlugin::new(sink.clone()));
    app.update();

    let slots = {
        let world = app.world_mut();
        let mut slots = world.query::<&OverviewCardSlot>();
        slots.iter(world).map(|slot| slot.0).collect::<Vec<_>>()
    };
    assert_eq!(
        slots.len(),
        OverviewCardKind::DEFAULT_ORDER.len(),
        "every shared card is mounted in its own reorderable slot"
    );

    let button = {
        let world = app.world_mut();
        let mut buttons = world.query::<(Entity, &OverviewCardMoveUpButton)>();
        buttons
            .iter(world)
            .find(|(_, marker)| marker.0 == OverviewCardKind::Traffic)
            .map(|(entity, _)| entity)
            .expect("the traffic card's move-up control")
    };
    app.world_mut()
        .commands()
        .trigger(Activate { entity: button });
    app.update();

    let submitted = sink.submitted();
    assert_eq!(submitted.len(), 1, "one move publishes one intent");
    match &submitted[0] {
        UiCommand::ReorderOverviewCards { order } => {
            assert_eq!(order.len(), OverviewCardKind::DEFAULT_ORDER.len());
            assert_eq!(order[0], OverviewCardKind::Traffic);
            assert_eq!(order[1], OverviewCardKind::RoutingIntentSegment);
        }
        other => panic!("the move publishes the shared reorder intent, got {other:?}"),
    }
}
