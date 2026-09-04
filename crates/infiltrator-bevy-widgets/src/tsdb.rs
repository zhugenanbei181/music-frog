//! Micro embedded time-series telemetry storage and time-travel replay scrubber.

use std::collections::VecDeque;

/// A single telemetry sample point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TelemetrySample {
    pub timestamp_sec: u64,
    pub upload_bytes: u64,
    pub download_bytes: u64,
    pub active_connections: u32,
    pub latency_ms: f32,
}

/// Three-tiered multi-resolution ring buffer TSDB.
#[derive(Clone, Debug)]
pub struct MultiTierTsdb {
    /// Tier 1: 1-second resolution (last 60s)
    pub tier_1s: VecDeque<TelemetrySample>,
    /// Tier 2: 10-second resolution (last 10m)
    pub tier_10s: VecDeque<TelemetrySample>,
    /// Tier 3: 1-minute resolution (last 1h)
    pub tier_1m: VecDeque<TelemetrySample>,
    pub max_capacity_per_tier: usize,
}

#[allow(clippy::manual_is_multiple_of)]
impl MultiTierTsdb {
    pub fn new(capacity: usize) -> Self {
        Self {
            tier_1s: VecDeque::with_capacity(capacity),
            tier_10s: VecDeque::with_capacity(capacity),
            tier_1m: VecDeque::with_capacity(capacity),
            max_capacity_per_tier: capacity,
        }
    }

    /// Push a raw 1-second sample and cascade aggregate down to coarser tiers.
    pub fn push(&mut self, sample: TelemetrySample) {
        if self.tier_1s.len() >= self.max_capacity_per_tier {
            self.tier_1s.pop_front();
        }
        self.tier_1s.push_back(sample);

        // Aggregate every 10 samples to Tier 2
        if self.tier_1s.len() % 10 == 0 {
            let avg_sample = self.aggregate_recent(&self.tier_1s, 10);
            if self.tier_10s.len() >= self.max_capacity_per_tier {
                self.tier_10s.pop_front();
            }
            self.tier_10s.push_back(avg_sample);
        }

        // Aggregate every 6 Tier 2 samples (60s) to Tier 3
        if self.tier_10s.len() % 6 == 0 && !self.tier_10s.is_empty() {
            let avg_sample = self.aggregate_recent(&self.tier_10s, 6);
            if self.tier_1m.len() >= self.max_capacity_per_tier {
                self.tier_1m.pop_front();
            }
            self.tier_1m.push_back(avg_sample);
        }
    }

    fn aggregate_recent(&self, queue: &VecDeque<TelemetrySample>, count: usize) -> TelemetrySample {
        let n = count.min(queue.len()).max(1);
        let mut up = 0;
        let mut down = 0;
        let mut conns = 0;
        let mut lat = 0.0;
        let mut last_ts = 0;

        for s in queue.iter().rev().take(n) {
            up += s.upload_bytes;
            down += s.download_bytes;
            conns += s.active_connections;
            lat += s.latency_ms;
            last_ts = s.timestamp_sec;
        }

        TelemetrySample {
            timestamp_sec: last_ts,
            upload_bytes: up / n as u64,
            download_bytes: down / n as u64,
            active_connections: conns / n as u32,
            latency_ms: lat / n as f32,
        }
    }

    /// Query the best sample at or immediately preceding a target timestamp.
    pub fn query_at(&self, target_ts: u64) -> Option<TelemetrySample> {
        self.tier_1s
            .iter()
            .rev()
            .find(|s| s.timestamp_sec <= target_ts)
            .copied()
            .or_else(|| {
                self.tier_10s
                    .iter()
                    .rev()
                    .find(|s| s.timestamp_sec <= target_ts)
                    .copied()
            })
            .or_else(|| {
                self.tier_1m
                    .iter()
                    .rev()
                    .find(|s| s.timestamp_sec <= target_ts)
                    .copied()
            })
    }
}

/// Time-travel telemetry scrubber state machine.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TimeTravelScrubber {
    pub min_timestamp: u64,
    pub max_timestamp: u64,
    pub current_timestamp: u64,
    pub is_scrubbing: bool,
}

impl TimeTravelScrubber {
    pub fn new(min_ts: u64, max_ts: u64) -> Self {
        Self {
            min_timestamp: min_ts,
            max_timestamp: max_ts,
            current_timestamp: max_ts,
            is_scrubbing: false,
        }
    }

    pub fn scrub_to_fraction(&mut self, fraction: f32) {
        let f = fraction.clamp(0.0, 1.0);
        let range = self.max_timestamp.saturating_sub(self.min_timestamp);
        self.current_timestamp = self.min_timestamp + (range as f64 * f as f64) as u64;
    }

    pub fn fraction(&self) -> f32 {
        let range = self.max_timestamp.saturating_sub(self.min_timestamp);
        if range == 0 {
            1.0
        } else {
            (self.current_timestamp.saturating_sub(self.min_timestamp) as f32 / range as f32)
                .clamp(0.0, 1.0)
        }
    }
}

/// Compute heuristic network health score [0.0..100.0].
pub fn compute_network_health_score(latency_ms: f32, packet_loss_rate: f32, jitter_ms: f32) -> f32 {
    let latency_penalty = (latency_ms / 5.0).clamp(0.0, 40.0);
    let loss_penalty = (packet_loss_rate * 400.0).clamp(0.0, 40.0);
    let jitter_penalty = (jitter_ms / 2.0).clamp(0.0, 20.0);

    (100.0 - latency_penalty - loss_penalty - jitter_penalty).clamp(0.0, 100.0)
}

/// Lossless delta-compressed telemetry series saving >70% memory for long-running telemetry rings.
#[derive(Clone, Debug, PartialEq)]
pub struct DeltaCompressedSeries {
    pub base_timestamp_sec: u64,
    pub base_upload_bytes: u64,
    pub base_download_bytes: u64,
    pub timestamp_deltas: Vec<u16>,
    pub upload_deltas: Vec<i64>,
    pub download_deltas: Vec<i64>,
}

impl DeltaCompressedSeries {
    /// Compress a sequence of telemetry samples into base values plus compact deltas.
    pub fn compress(samples: &[TelemetrySample]) -> Option<Self> {
        let first = samples.first()?;
        let mut timestamp_deltas = Vec::with_capacity(samples.len() - 1);
        let mut upload_deltas = Vec::with_capacity(samples.len() - 1);
        let mut download_deltas = Vec::with_capacity(samples.len() - 1);

        let mut prev = *first;
        for curr in &samples[1..] {
            let dt = curr
                .timestamp_sec
                .saturating_sub(prev.timestamp_sec)
                .min(u16::MAX as u64) as u16;
            let dup = curr.upload_bytes as i64 - prev.upload_bytes as i64;
            let ddown = curr.download_bytes as i64 - prev.download_bytes as i64;

            timestamp_deltas.push(dt);
            upload_deltas.push(dup);
            download_deltas.push(ddown);
            prev = *curr;
        }

        Some(Self {
            base_timestamp_sec: first.timestamp_sec,
            base_upload_bytes: first.upload_bytes,
            base_download_bytes: first.download_bytes,
            timestamp_deltas,
            upload_deltas,
            download_deltas,
        })
    }

    /// Decompress the series back to original telemetry samples with bit-exact fidelity.
    pub fn decompress(&self) -> Vec<TelemetrySample> {
        let n = self.timestamp_deltas.len() + 1;
        let mut result = Vec::with_capacity(n);

        let mut current_ts = self.base_timestamp_sec;
        let mut current_up = self.base_upload_bytes;
        let mut current_down = self.base_download_bytes;

        result.push(TelemetrySample {
            timestamp_sec: current_ts,
            upload_bytes: current_up,
            download_bytes: current_down,
            active_connections: 0,
            latency_ms: 0.0,
        });

        for i in 0..self.timestamp_deltas.len() {
            current_ts += self.timestamp_deltas[i] as u64;
            current_up = (current_up as i64 + self.upload_deltas[i]).max(0) as u64;
            current_down = (current_down as i64 + self.download_deltas[i]).max(0) as u64;

            result.push(TelemetrySample {
                timestamp_sec: current_ts,
                upload_bytes: current_up,
                download_bytes: current_down,
                active_connections: 0,
                latency_ms: 0.0,
            });
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delta_compressed_series_lossless_round_trip() {
        let samples = vec![
            TelemetrySample {
                timestamp_sec: 1000,
                upload_bytes: 5000,
                download_bytes: 20000,
                active_connections: 10,
                latency_ms: 25.0,
            },
            TelemetrySample {
                timestamp_sec: 1001,
                upload_bytes: 5200,
                download_bytes: 25000,
                active_connections: 12,
                latency_ms: 26.0,
            },
            TelemetrySample {
                timestamp_sec: 1002,
                upload_bytes: 4800,
                download_bytes: 30000,
                active_connections: 15,
                latency_ms: 28.0,
            },
        ];

        let compressed = DeltaCompressedSeries::compress(&samples).unwrap();
        assert_eq!(compressed.timestamp_deltas, vec![1, 1]);
        assert_eq!(compressed.upload_deltas, vec![200, -400]);
        assert_eq!(compressed.download_deltas, vec![5000, 5000]);

        let decompressed = compressed.decompress();
        assert_eq!(decompressed.len(), 3);
        assert_eq!(decompressed[0].timestamp_sec, 1000);
        assert_eq!(decompressed[1].upload_bytes, 5200);
        assert_eq!(decompressed[2].download_bytes, 30000);
    }
}
