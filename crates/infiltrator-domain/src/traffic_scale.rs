//! Pure dynamic scale and unit policy for traffic telemetry.

use infiltrator_contract::traffic_scale::{TrafficRateUnit, TrafficScaleSnapshot};
use infiltrator_contract::traffic_waveform::TrafficWaveformSnapshot;

pub const SCALE_HEADROOM: f64 = 0.05;

pub fn compute(snapshot: &TrafficWaveformSnapshot, revision: u64) -> TrafficScaleSnapshot {
    let peak = snapshot
        .samples
        .iter()
        .flat_map(|sample| [sample.upload_bps, sample.download_bps])
        .filter(|value| value.is_finite() && *value >= 0.0)
        .fold(0.0_f64, f64::max);
    compute_from_peak(peak, revision)
}

pub fn compute_from_peak(peak_bps: f64, revision: u64) -> TrafficScaleSnapshot {
    let peak_bps = if peak_bps.is_finite() && peak_bps >= 0.0 {
        peak_bps
    } else {
        0.0
    };
    let max_bps = if peak_bps > 0.0 {
        peak_bps * (1.0 + SCALE_HEADROOM)
    } else {
        1.0
    };
    let unit = if peak_bps >= TrafficRateUnit::GibiBytes.factor() {
        TrafficRateUnit::GibiBytes
    } else if peak_bps >= TrafficRateUnit::MebiBytes.factor() {
        TrafficRateUnit::MebiBytes
    } else if peak_bps >= TrafficRateUnit::KibiBytes.factor() {
        TrafficRateUnit::KibiBytes
    } else {
        TrafficRateUnit::Bytes
    };
    TrafficScaleSnapshot {
        peak_bps,
        max_bps,
        unit,
        unit_factor: unit.factor(),
        ticks: vec![0.0, 0.25, 0.5, 0.75, 1.0],
        revision,
    }
}

pub fn compute_from_rates(
    upload_bps: &[f64],
    download_bps: &[f64],
    revision: u64,
) -> TrafficScaleSnapshot {
    let peak = upload_bps
        .iter()
        .chain(download_bps)
        .filter(|value| value.is_finite() && **value >= 0.0)
        .copied()
        .fold(0.0_f64, f64::max);
    compute_from_peak(peak, revision)
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::traffic_waveform::TrafficSample;

    #[test]
    fn scale_uses_the_larger_channel_and_selects_a_human_unit() {
        let snapshot = TrafficWaveformSnapshot {
            generation: 1,
            revision: 2,
            samples: vec![TrafficSample {
                sampled_at_epoch_ms: None,
                upload_bps: 2.0 * 1024.0 * 1024.0,
                download_bps: 5.0 * 1024.0 * 1024.0,
            }],
        };
        let scale = compute(&snapshot, 3);
        assert_eq!(scale.unit, TrafficRateUnit::MebiBytes);
        assert_eq!(scale.peak_bps, 5.0 * 1024.0 * 1024.0);
        assert!(scale.max_bps > scale.peak_bps);
        assert_eq!(scale.format_tick(1.0), scale.format_max());
    }

    #[test]
    fn zero_nonfinite_and_negative_rates_have_a_safe_baseline() {
        let scale = compute_from_rates(&[f64::NAN, -1.0], &[f64::INFINITY, 0.0], 4);
        assert_eq!(scale.peak_bps, 0.0);
        assert_eq!(scale.max_bps, 1.0);
        assert_eq!(scale.unit, TrafficRateUnit::Bytes);
    }
}
