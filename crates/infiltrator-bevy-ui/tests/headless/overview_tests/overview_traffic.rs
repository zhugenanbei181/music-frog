//! Behavior cases for overview traffic.
//! test-intent: behavior

use super::*;

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
