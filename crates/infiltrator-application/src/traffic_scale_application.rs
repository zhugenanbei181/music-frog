//! Application facade for the shared traffic scale projection.

use infiltrator_contract::traffic_scale::TrafficScaleSnapshot;
use infiltrator_contract::traffic_waveform::TrafficWaveformSnapshot;

#[derive(Clone, Copy, Debug, Default)]
pub struct TrafficScaleApplication;

impl TrafficScaleApplication {
    pub fn compute(&self, waveform: &TrafficWaveformSnapshot) -> TrafficScaleSnapshot {
        infiltrator_domain::traffic_scale::compute(waveform, waveform.revision)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::traffic_scale::TrafficRateUnit;
    use infiltrator_contract::traffic_waveform::TrafficSample;

    #[test]
    fn application_exposes_one_scale_for_both_channels() {
        let waveform = TrafficWaveformSnapshot {
            generation: 1,
            revision: 9,
            samples: vec![TrafficSample {
                sampled_at_epoch_ms: None,
                upload_bps: 100.0,
                download_bps: 10_000.0,
            }],
        };
        let scale = TrafficScaleApplication.compute(&waveform);
        assert_eq!(scale.unit, TrafficRateUnit::KibiBytes);
        assert_eq!(scale.revision, 9);
    }
}
