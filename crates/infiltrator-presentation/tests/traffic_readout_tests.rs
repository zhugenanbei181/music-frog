use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::shell_readout::ShellObservation;
use infiltrator_contract::shell_readout::ShellReadoutSnapshot;
use infiltrator_contract::traffic_waveform::{TrafficSample, TrafficWaveformSnapshot};
use infiltrator_presentation::shell_readout::observed_rate;
use infiltrator_presentation::traffic_readout::{TrafficFreshness, project_traffic_readout};

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
    assert_eq!(unknown.freshness, TrafficFreshness::Unobserved);
    assert!(unknown.upload_peak.is_none());

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
    assert_eq!(zero.freshness, TrafficFreshness::Current);

    rates.upload_bps = ShellObservation::Stale(0.0);
    rates.download_bps = ShellObservation::Stale(0.0);
    let stale = project_traffic_readout(&rates, &history, "zh-CN");
    assert_eq!(stale.upload, "0 B/s（已失效）");
    assert_eq!(stale.status, "已失效");
    assert_eq!(stale.upload_peak.as_deref(), Some("0 B/s（已失效）"));
    assert_eq!(stale.freshness, TrafficFreshness::Stale);

    history.generation = 6;
    let retired = project_traffic_readout(&rates, &history, "en-US");
    assert!(retired.upload_peak.is_none() && retired.download_peak.is_none());
    assert_eq!(retired.freshness, TrafficFreshness::Stale);
}

#[test]
fn failure_is_a_typed_terminal_even_when_last_values_remain_visible() {
    let mut rates = ShellReadoutSnapshot {
        upload_bps: observed_rate(Some(5.0)),
        download_bps: observed_rate(Some(7.0)),
        ..Default::default()
    };
    rates.rate_failure = Some(Failure::new(ErrorCode::Permission, "denied", false));

    let readout = project_traffic_readout(&rates, &TrafficWaveformSnapshot::default(), "en-US");
    assert_eq!(readout.freshness, TrafficFreshness::Failed);
    assert_eq!(readout.upload, "5 B/s");
}
