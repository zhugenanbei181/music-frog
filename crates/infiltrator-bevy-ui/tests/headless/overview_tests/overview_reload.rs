//! Behavior cases for overview reload.
//! test-intent: behavior

use super::*;
use infiltrator_contract::reconnect_mask::ReconnectMaskSnapshot;

#[test]
fn overview_reload_mask_activates_and_preserves_facts() {
    let mut app = mounted_default();
    let mask_entity = {
        let world = app.world_mut();
        let mut masks = world.query::<(Entity, &OverviewReloadMask)>();
        masks.single(world).expect("reload mask").0
    };
    assert!(mask_entity != Entity::PLACEHOLDER);

    {
        let world = app.world_mut();
        let node = world.get::<Node>(mask_entity).expect("node on mask");
        assert_eq!(node.display, Display::None);
    }

    let mut projection = DemoOverviewSource::running().current();
    projection.reconnect_mask = ReconnectMaskSnapshot::reloading("配置热重载中，保持画面");
    app.world_mut()
        .commands()
        .trigger(OverviewProjectionUpdated(projection));
    app.update();

    let world = app.world_mut();
    let node = world.get::<Node>(mask_entity).expect("node on mask");
    assert_eq!(node.display, Display::Flex);

    let mut texts = world.query::<(&OverviewReloadMaskText, &Text)>();
    assert!(
        texts
            .iter(world)
            .any(|(_, text)| text.0 == "配置热重载中，保持画面")
    );
}
