//! test-intent: behavior
use crate::history::{TrafficHistory, chart_inputs};
use crate::projection::{DemoOverviewSource, OverviewOrigin, OverviewSource};
use infiltrator_contract::traffic_waveform::TrafficSample;

#[test]
fn empty_new_source_never_resurrects_previous_native_history_and_one_measured_zero_is_retained() {
    let mut projection = DemoOverviewSource::running().current();
    projection.origin = OverviewOrigin::LiveCore;
    let mut history = TrafficHistory::default();
    history.push(8192.0, 4096.0);
    let (upload, download, _, _) = chart_inputs(&projection, &history);
    assert!(upload.is_empty() && download.is_empty());
    projection.traffic_waveform.samples.push(TrafficSample {
        sampled_at_epoch_ms: Some(1),
        upload_bps: 0.0,
        download_bps: 0.0,
    });
    let (upload, download, _, _) = chart_inputs(&projection, &history);
    assert_eq!(upload, vec![0.0]);
    assert_eq!(download, vec![0.0]);
}
