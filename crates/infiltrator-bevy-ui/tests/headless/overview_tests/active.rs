//! Behavior cases for active.
//! test-intent: behavior

use super::*;
use infiltrator_contract::active_exit::ActiveExitSnapshot;

#[test]
fn active_exit_projection_restsamps_facts_and_failure_in_place() {
    let mut app = mounted_default();
    let exit_id = {
        let world = app.world_mut();
        let mut exits = world.query::<(Entity, &ActiveExitNodeCard)>();
        exits.single(world).expect("active exit card").0
    };
    let mut projection = DemoOverviewSource::running().current();
    projection.active_exit = ActiveExitSnapshot::failed(1, 2, "proxy read failed");
    app.world_mut()
        .commands()
        .trigger(OverviewProjectionUpdated(projection));
    app.update();

    let world = app.world_mut();
    let mut facts = world.query::<(&ActiveExitText, &Text)>();
    assert!(facts.iter(world).any(|(marker, text)| {
        marker.0 == ActiveExitTextKind::Status && text.0 == "proxy read failed"
    }));
    assert!(world.get_entity(exit_id).is_ok(), "exit card stays mounted");
}
