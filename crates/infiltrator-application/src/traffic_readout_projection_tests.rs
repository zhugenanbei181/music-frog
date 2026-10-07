//! test-intent: behavior
use super::*;
use crate::shell_readout_projection::observed_rate;
use infiltrator_contract::traffic_waveform::TrafficSample;

#[test]
fn absent_zero_retained_and_wrong_generation_peaks_have_distinct_copy() {
    let mut rates = ShellReadoutSnapshot {
        generation: 7,
        ..Default::default()
    };
    let mut history = TrafficWaveformSnapshot {
        generation: 7,
        ..Default::default()
    };
    let unknown = project_traffic_readout(&rates, &history, "en-US");
    assert_eq!(unknown.upload, "Not observed");
    assert_eq!(unknown.status, "Not observed");
    assert!(!unknown.current && !unknown.observed && unknown.upload_peak.is_none());

    rates.upload_bps = observed_rate(Some(0.0));
    rates.download_bps = observed_rate(Some(0.0));
    history.samples.push(TrafficSample {
        sampled_at_epoch_ms: Some(1),
        upload_bps: 0.0,
        download_bps: 0.0,
    });
    let zero = project_traffic_readout(&rates, &history, "en-US");
    assert_eq!(zero.upload, "0 B/s");
    assert_eq!(zero.status, "Live");
    assert_eq!(zero.upload_peak.as_deref(), Some("0 B/s"));
    assert!(zero.current && zero.observed);

    rates.upload_bps.current = false;
    rates.download_bps.current = false;
    let stale = project_traffic_readout(&rates, &history, "zh-CN");
    assert_eq!(stale.upload, "0 B/s（已失效）");
    assert_eq!(stale.status, "已失效");
    assert_eq!(stale.upload_peak.as_deref(), Some("0 B/s（已失效）"));
    assert!(!stale.current && stale.observed);

    history.generation = 6;
    let retired = project_traffic_readout(&rates, &history, "en-US");
    assert!(retired.upload_peak.is_none() && retired.download_peak.is_none());
}
