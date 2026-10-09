//! Behavior cases for overview traffic.
//! test-intent: behavior

use super::*;
use infiltrator_bevy_ui::history::{ScrubberAction, TrafficHistory};
use infiltrator_bevy_ui::pages::overview::{OverviewScrubberButton, OverviewScrubberStatus};
use infiltrator_domain::traffic_waveform::display_series;

/// Hovering the traffic chart activates interactive crosshair inspection
/// with snapped sample instantaneous rates (UI-04-04).
#[test]
fn overview_traffic_chart_crosshair_hover_activation() {
    let mut app = mounted_default();
    let (plate_id, _) = chart_plate(app.world_mut());

    // Verify ChartCrosshairTracked is attached on the chart entity
    assert!(
        app.world().get::<ChartCrosshairTracked>(plate_id).is_some(),
        "chart node must have ChartCrosshairTracked"
    );

    // Initial state: crosshair is None
    let plate_before = app.world().get::<ChartPlate>(plate_id).unwrap();
    assert!(plate_before.0.crosshair.is_none());

    // Simulate Hovered pointer interaction
    app.world_mut()
        .entity_mut(plate_id)
        .insert(PickingInteraction::Hovered);
    app.update();

    let plate_hovered = app.world().get::<ChartPlate>(plate_id).unwrap();
    let crosshair = plate_hovered
        .0
        .crosshair
        .as_ref()
        .expect("hovering the chart must activate crosshair inspection");
    assert!(crosshair.active);
    let snapped_idx = crosshair
        .snapped_index
        .expect("crosshair should snap to nearest sample point");

    // Compute instantaneous rates from snapped sample index
    let instant = compute_instant_rates(&plate_hovered.0.up, &plate_hovered.0.down, snapped_idx)
        .expect("must extract valid instant rates");
    assert!(instant.upload_bps >= 0.0);
    assert!(instant.download_bps >= 0.0);

    // Simulate pointer leaving (None interaction)
    app.world_mut()
        .entity_mut(plate_id)
        .insert(PickingInteraction::None);
    app.update();

    let plate_after = app.world().get::<ChartPlate>(plate_id).unwrap();
    assert!(
        plate_after.0.crosshair.is_none(),
        "leaving hover must dismiss crosshair"
    );
}

/// The scrubber button time-travels the chart to a frozen historical window
/// in place, and the return-to-live button restores the live series — same
/// chart entity, no remount (BEVY-036).
#[test]
fn overview_scrubber_seeks_history_and_returns_to_live() {
    let mut app = mounted_app_with(LiveFootStub { version: None });
    let mut projection = live_projection(111.0, 222.0);
    projection.traffic_waveform.samples = vec![
        TrafficSample {
            sampled_at_epoch_ms: Some(1),
            upload_bps: 5.0,
            download_bps: 6.0,
        },
        TrafficSample {
            sampled_at_epoch_ms: Some(2),
            upload_bps: 7.0,
            download_bps: 8.0,
        },
    ];
    let live_series = display_series(&projection.traffic_waveform);
    {
        let mut history = app.world_mut().resource_mut::<TrafficHistory>();
        for tick in 0..30u64 {
            history.push(tick as f64 * 10.0, tick as f64 * 20.0);
        }
    }
    app.world_mut()
        .commands()
        .trigger(OverviewProjectionUpdated(projection));
    app.update();

    let (chart_id, plate) = chart_plate(app.world_mut());
    assert_eq!(
        (plate.0.up.clone(), plate.0.down.clone()),
        live_series,
        "the live waveform mounts first"
    );

    let step_back = scrubber_button(app.world_mut(), ScrubberAction::StepBackward);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: step_back });
    app.update();

    let (same_id, scrubbed) = chart_plate(app.world_mut());
    assert_eq!(same_id, chart_id, "the chart never remounts");
    assert_ne!(
        (scrubbed.0.up.clone(), scrubbed.0.down.clone()),
        live_series,
        "scrubbing shows the historical window, not the live series"
    );
    assert_eq!(
        scrubbed.0.up.last(),
        Some(&280.0),
        "one step back lands on retained sample 28"
    );
    assert_eq!(scrubber_status(app.world_mut()), "⏪ 29/30");

    let return_to_live = scrubber_button(app.world_mut(), ScrubberAction::ReturnToLive);
    app.world_mut().commands().trigger(Activate {
        entity: return_to_live,
    });
    app.update();

    let (_, resumed) = chart_plate(app.world_mut());
    assert_eq!(
        (resumed.0.up, resumed.0.down),
        live_series,
        "return-to-live resumes the live window"
    );
}

fn scrubber_button(world: &mut World, action: ScrubberAction) -> Entity {
    let mut buttons = world.query::<(Entity, &OverviewScrubberButton)>();
    buttons
        .iter(world)
        .find(|(_, button)| button.0 == action)
        .map(|(entity, _)| entity)
        .expect("the scrubber button is mounted")
}

fn scrubber_status(world: &mut World) -> String {
    world
        .query::<(&OverviewScrubberStatus, &Text)>()
        .iter(world)
        .next()
        .map(|(_, text)| text.0.clone())
        .expect("the scrubber status is mounted")
}
