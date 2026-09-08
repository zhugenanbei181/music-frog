//! Runtime-neutral traffic sampling and cubic Bezier value projection.

use infiltrator_contract::snapshot::{CoreLifecycle, CoreSnapshot};
use infiltrator_contract::traffic_waveform::{
    TRAFFIC_WAVEFORM_CAPACITY, TrafficSample, TrafficWaveformSnapshot,
};
use std::collections::VecDeque;

/// Application-owned bounded live sample history. UI toolkits only receive
/// the immutable contract snapshot and never own sampling policy.
#[derive(Clone, Debug, Default)]
pub struct TrafficWaveformBuffer {
    generation: Option<u64>,
    revision: u64,
    samples: VecDeque<TrafficSample>,
}

impl TrafficWaveformBuffer {
    pub fn record(&mut self, core: &CoreSnapshot) -> TrafficWaveformSnapshot {
        if self.generation != Some(core.generation) {
            self.generation = Some(core.generation);
            self.samples.clear();
        }
        if matches!(
            core.lifecycle,
            CoreLifecycle::Running | CoreLifecycle::Ready
        ) {
            if self.samples.len() == TRAFFIC_WAVEFORM_CAPACITY {
                self.samples.pop_front();
            }
            self.samples.push_back(TrafficSample {
                sampled_at_epoch_ms: core.sampled_at_epoch_ms,
                upload_bps: sanitize_rate(core.upload_bps),
                download_bps: sanitize_rate(core.download_bps),
            });
            self.revision = self.revision.saturating_add(1);
        }
        self.snapshot()
    }

    pub fn snapshot(&self) -> TrafficWaveformSnapshot {
        TrafficWaveformSnapshot {
            generation: self.generation.unwrap_or_default(),
            revision: self.revision,
            samples: self.samples.iter().cloned().collect(),
        }
    }
}

fn sanitize_rate(rate: f64) -> f64 {
    if rate.is_finite() && rate >= 0.0 {
        rate
    } else {
        0.0
    }
}

/// Project values to a dense cubic-Bezier curve. The controls are clamped to
/// each segment's endpoints so a burst cannot create a false negative rate.
pub fn smooth_series(samples: &[f64], subdivisions_per_segment: usize) -> Vec<f32> {
    let values: Vec<f64> = samples.iter().copied().map(sanitize_rate).collect();
    if values.len() < 2 {
        return values.into_iter().map(|value| value as f32).collect();
    }
    let steps = subdivisions_per_segment.max(1);
    let mut output = Vec::with_capacity((values.len() - 1) * steps + 1);
    for index in 0..values.len() - 1 {
        let p0 = values[index];
        let p1 = values[index + 1];
        let previous = if index == 0 { p0 } else { values[index - 1] };
        let next = if index + 2 < values.len() {
            values[index + 2]
        } else {
            p1
        };
        let c1 = p0 + (p1 - previous) / 6.0;
        let c2 = p1 - (next - p0) / 6.0;
        for step in 0..steps {
            let t = step as f64 / steps as f64;
            let u = 1.0 - t;
            let value =
                u * u * u * p0 + 3.0 * u * u * t * c1 + 3.0 * u * t * t * c2 + t * t * t * p1;
            output.push(value.clamp(p0.min(p1), p0.max(p1)) as f32);
        }
    }
    output.push(*values.last().unwrap() as f32);
    output
}

pub fn smooth_dual_series(
    upload: &[f64],
    download: &[f64],
    subdivisions_per_segment: usize,
) -> (Vec<f32>, Vec<f32>) {
    (
        smooth_series(upload, subdivisions_per_segment),
        smooth_series(download, subdivisions_per_segment),
    )
}

pub fn display_series(snapshot: &TrafficWaveformSnapshot) -> (Vec<f32>, Vec<f32>) {
    let upload: Vec<f64> = snapshot
        .samples
        .iter()
        .map(|sample| sample.upload_bps)
        .collect();
    let download: Vec<f64> = snapshot
        .samples
        .iter()
        .map(|sample| sample.download_bps)
        .collect();
    smooth_dual_series(&upload, &download, 4)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn core(generation: u64, upload: f64, download: f64, lifecycle: CoreLifecycle) -> CoreSnapshot {
        CoreSnapshot {
            lifecycle,
            generation,
            session_token: None,
            revision: 0,
            proxy_mode: None,
            core_version: None,
            sampled_at_epoch_ms: None,
            failure: None,
            upload_bps: upload,
            download_bps: download,
            active_connections: 0,
            memory_bytes: None,
            watchdog: Default::default(),
        }
    }

    #[test]
    fn buffer_is_bounded_and_resets_on_generation_change() {
        let mut buffer = TrafficWaveformBuffer::default();
        for value in 0..(TRAFFIC_WAVEFORM_CAPACITY + 3) {
            buffer.record(&core(
                1,
                value as f64,
                (value * 2) as f64,
                CoreLifecycle::Running,
            ));
        }
        assert_eq!(buffer.snapshot().samples.len(), TRAFFIC_WAVEFORM_CAPACITY);
        let reset = buffer.record(&core(2, 9.0, 11.0, CoreLifecycle::Running));
        assert_eq!(reset.generation, 2);
        assert_eq!(reset.samples.len(), 1);
    }

    #[test]
    fn smoothing_is_finite_monotonic_per_segment_and_dual() {
        let values = smooth_series(&[0.0, 100.0, 20.0], 4);
        assert_eq!(values.len(), 9);
        assert!(values.iter().all(|value| value.is_finite()));
        assert!(
            values[..4]
                .iter()
                .all(|value| (0.0..=100.0).contains(value))
        );
        let (upload, download) = smooth_dual_series(&[1.0, 2.0], &[4.0, 8.0], 2);
        assert_eq!(upload.len(), 3);
        assert_eq!(download.len(), 3);
    }

    #[test]
    fn stopped_core_does_not_append_a_fake_zero_sample() {
        let mut buffer = TrafficWaveformBuffer::default();
        buffer.record(&core(1, 10.0, 20.0, CoreLifecycle::Running));
        let stopped = buffer.record(&core(1, 0.0, 0.0, CoreLifecycle::Stopped));
        assert_eq!(stopped.samples.len(), 1);
    }
}
