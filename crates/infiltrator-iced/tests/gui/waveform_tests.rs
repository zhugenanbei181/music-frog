use super::*;
use infiltrator_contract::traffic_waveform::{TrafficSample, TrafficWaveformSnapshot};

#[test]
fn shared_live_samples_are_the_input_to_the_same_bezier_projection() {
    let chart = TrafficChart {
        history: VecDeque::new(),
        shared: Some(TrafficWaveformSnapshot {
            generation: 1,
            revision: 2,
            samples: vec![
                TrafficSample {
                    sampled_at_epoch_ms: None,
                    upload_bps: 1.0,
                    download_bps: 2.0,
                },
                TrafficSample {
                    sampled_at_epoch_ms: None,
                    upload_bps: 3.0,
                    download_bps: 5.0,
                },
            ],
        }),
        scale: None,
    };
    let (upload, download) = chart.raw_series();
    let (smooth_upload, smooth_download) =
        infiltrator_domain::traffic_waveform::smooth_dual_series(&upload, &download, 4);
    assert_eq!(upload, vec![1.0, 3.0]);
    assert_eq!(download, vec![2.0, 5.0]);
    assert_eq!(smooth_upload.len(), 5);
    assert_eq!(smooth_download.len(), 5);
}

#[test]
fn canvas_uses_the_application_scale_instead_of_a_fixed_floor() {
    let chart = TrafficChart {
        history: VecDeque::from([(1, 2), (3, 5)]),
        shared: None,
        scale: Some(infiltrator_domain::traffic_scale::compute_from_peak(
            20_000.0, 7,
        )),
    };
    let (upload, download) = chart.raw_series();
    let scale = chart.resolved_scale(&upload, &download);
    assert_eq!(scale.revision, 7);
    assert_eq!(scale.peak_bps, 20_000.0);
    assert!(scale.max_bps > 20_000.0);
}

#[test]
fn test_traffic_chart_crosshair_and_hud_tooltip_geometry() {
    let chart = TrafficChart {
        history: VecDeque::from([
            (1_000_000, 5_000_000),
            (2_000_000, 8_000_000),
            (3_000_000, 10_000_000),
        ]),
        shared: None,
        scale: None,
    };
    let (up, down) = chart.raw_series();
    assert_eq!(up.len(), 3);
    assert_eq!(down.len(), 3);

    // Verify rate formatting in HUD
    let up_fmt = format!("↑ {}/s", crate::utils::format_bytes(up[1] as u64));
    let down_fmt = format!("↓ {}/s", crate::utils::format_bytes(down[1] as u64));
    assert!(up_fmt.contains("MB") || up_fmt.contains("KB"));
    assert!(down_fmt.contains("MB") || down_fmt.contains("KB"));
}
