//! Behavior cases for traffic.
//! test-intent: behavior

use super::*;

/// The page's trend band mounts a real chart node: the plate carries the fixed
/// token box and the demo fixture's synthetic series, lives inside the page's
/// own trend band rather than inside one card slot, and has actually been
/// rasterized (the widget's sync_charts stamped its ImageNode).
#[test]
fn trend_band_mounts_the_rasterized_trend_chart() {
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

    let mut bands = world.query::<(Entity, &OverviewTrafficDetailSection)>();
    let (band, _) = bands
        .iter(world)
        .next()
        .expect("the page's trend band mounts");
    assert!(
        descendants(world, band).contains(&plate_id),
        "the chart node lives inside the page's trend band"
    );
    assert!(
        world.get::<ImageNode>(plate_id).is_some(),
        "sync_charts rasterized the plate on mount"
    );
}

/// The overview traffic slot mounts two independent live-monitor waveform
/// cards: one upload plate and one download plate, both fed from the
/// authoritative chart series.
#[test]
fn traffic_slot_mounts_independent_upload_and_download_waveforms() {
    let mut app = mounted_default();
    let world = app.world_mut();
    let mut plates = world.query::<&WaveformPlate>();
    let series: Vec<_> = plates.iter(world).map(|plate| plate.series).collect();
    assert!(series.contains(&WaveformSeries::Upload));
    assert!(series.contains(&WaveformSeries::Download));
    assert_eq!(series.len(), 2, "exactly two waveform plates mount");
    for plate in plates.iter(world) {
        assert!(
            !plate.samples.is_empty(),
            "the demo fixture supplies a real series for every mini card"
        );
    }
}

/// The two mini readings share one wrapping row with a rem width basis, so a
/// narrow slot stacks them instead of squeezing a rate into two broken lines.
/// The tablet capture proved the old non-wrapping row broke `1.40 MB/s` in the
/// middle of its unit.
#[test]
fn traffic_mini_readings_wrap_instead_of_breaking_the_rate() {
    use bevy::ui::FlexWrap;
    use infiltrator_bevy_widgets::palette::rem;

    let mut app = mounted_default();
    let world = app.world_mut();
    let basis = rem(176.0);
    let wrappers: Vec<Entity> = {
        let mut query = world.query::<(Entity, &Node)>();
        query
            .iter(world)
            .filter(|(_, node)| node.flex_basis == basis && node.flex_grow == 1.0)
            .map(|(entity, _)| entity)
            .collect()
    };
    assert_eq!(
        wrappers.len(),
        2,
        "one rem-basis wrapper per mini reading, got {wrappers:?}"
    );

    let rows: Vec<Entity> = wrappers
        .iter()
        .map(|wrapper| {
            world
                .get::<ChildOf>(*wrapper)
                .expect("the mini slot is parented")
                .0
        })
        .collect();
    assert_eq!(rows[0], rows[1], "both mini readings share one row");
    let row = rows[0];

    assert_eq!(
        world.get::<Node>(row).expect("the row node").flex_wrap,
        FlexWrap::Wrap,
        "the pair must stack when the slot is narrow"
    );
    let children = world.get::<Children>(row).expect("the row has children");
    assert_eq!(children.len(), 2, "the row holds the two mini readings");
}
