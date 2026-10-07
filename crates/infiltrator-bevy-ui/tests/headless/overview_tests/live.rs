//! Behavior cases for live.
//! test-intent: behavior

use super::*;
use infiltrator_bevy_widgets::chart::bezier::ScaleMode;
use infiltrator_contract::traffic_scale::{TrafficRateUnit, TrafficScaleSnapshot};

#[test]
fn live_surface_waveform_uses_shared_bezier_value_projection() {
    let mut app = mounted_default();
    let mut projection = live_projection(5.0, 6.0);
    projection.traffic_waveform = TrafficWaveformSnapshot {
        generation: 2,
        revision: 3,
        samples: vec![
            TrafficSample {
                sampled_at_epoch_ms: Some(1),
                upload_bps: 1.0,
                download_bps: 3.0,
            },
            TrafficSample {
                sampled_at_epoch_ms: Some(2),
                upload_bps: 9.0,
                download_bps: 6.0,
            },
            TrafficSample {
                sampled_at_epoch_ms: Some(3),
                upload_bps: 4.0,
                download_bps: 12.0,
            },
        ],
    };
    projection.traffic_scale = TrafficScaleSnapshot {
        peak_bps: 12.0,
        max_bps: 12.6,
        unit: TrafficRateUnit::Bytes,
        unit_factor: 1.0,
        ticks: vec![0.0, 0.5, 1.0],
        revision: 3,
    };
    app.world_mut()
        .commands()
        .trigger(OverviewProjectionUpdated(projection));
    app.update();

    let (_, plate) = chart_plate(app.world_mut());
    assert_eq!(plate.0.up.len(), 9, "three live samples, four Bezier steps");
    assert_eq!(plate.0.down.len(), 9);
    assert!(
        !plate.0.smooth,
        "shared adapter already densified the values"
    );
    assert!(matches!(
        plate.0.scale_mode,
        ScaleMode::Fixed(value)
            if (value - 12.6).abs() < f32::EPSILON
    ));
}
