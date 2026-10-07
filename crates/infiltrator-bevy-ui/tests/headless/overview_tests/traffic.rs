//! Behavior cases for traffic.
//! test-intent: behavior

use super::*;

/// The traffic card mounts a real chart node: the plate carries the fixed
/// token box, the demo fixture's synthetic series, lives inside the card
/// whose caption is 实时流量, and has actually been rasterized (the
/// widget's sync_charts stamped its ImageNode).
#[test]
fn traffic_card_mounts_the_trend_chart() {
    let mut app = mounted_default();
    app.update(); // the frame sync_charts stamps the raster on
    let world = app.world_mut();
    let (plate_id, plate) = chart_plate(world);

    assert_eq!(
        plate.0.width,
        CHART_WIDTH_PX.round() as u32,
        "full-card box"
    );
    assert_eq!(plate.0.height, CHART_HEIGHT_PX.round() as u32);
    let (demo_up, demo_down) = demo_traffic_series();
    assert_eq!(plate.0.up, demo_up, "demo origin draws the fixture waves");
    assert_eq!(plate.0.down, demo_down);

    let mut cards = world.query::<(Entity, &SurfacePanel)>();
    let card_ids: Vec<Entity> = cards.iter(world).map(|(id, _)| id).collect();
    let traffic_card = card_ids
        .iter()
        .copied()
        .find(|id| {
            descendants(world, *id)
                .iter()
                .any(|e| world.get::<Text>(*e).is_some_and(|t| t.0 == "实时流量"))
        })
        .expect("the traffic card is mounted");
    assert!(
        descendants(world, traffic_card).contains(&plate_id),
        "the chart node lives inside the traffic card"
    );
    assert!(
        world.get::<ImageNode>(plate_id).is_some(),
        "sync_charts rasterized the plate on mount"
    );
}
