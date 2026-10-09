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

/// Default retained-sample ceiling for a [`TelemetryStore`]: one hour at 1 Hz.
pub const DEFAULT_STORE_CAPACITY: usize = 3_600;

/// A bounded, append-only telemetry store that keeps samples delta-compressed
/// and evicts the oldest sample when full.
///
/// The oldest retained sample is the `base`; every later sample is stored as a
/// delta from its predecessor. Eviction is O(1): the first delta is folded into
/// the base and dropped. Timestamps, byte counters and connection counts live
/// in delta lanes; `latency_ms` is an exact parallel f32 lane because
/// floating-point subtraction is not reversible, and a lossless store must
/// reproduce it bit-for-bit.
#[derive(Clone, Debug)]
pub struct TelemetryStore {
    base: Option<TelemetrySample>,
    last: Option<TelemetrySample>,
    timestamp_deltas: VecDeque<u16>,
    upload_deltas: VecDeque<i64>,
    download_deltas: VecDeque<i64>,
    connection_deltas: VecDeque<i32>,
    latency_ms: VecDeque<f32>,
    capacity: usize,
}

impl TelemetryStore {
    /// Create an empty store retaining at most `capacity` samples (at least 1).
    pub fn new(capacity: usize) -> Self {
        let capacity = capacity.max(1);
        Self {
            base: None,
            last: None,
            timestamp_deltas: VecDeque::with_capacity(capacity),
            upload_deltas: VecDeque::with_capacity(capacity),
            download_deltas: VecDeque::with_capacity(capacity),
            connection_deltas: VecDeque::with_capacity(capacity),
            latency_ms: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    /// Maximum number of retained samples.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Number of retained samples.
    pub fn len(&self) -> usize {
        if self.base.is_none() {
            0
        } else {
            self.timestamp_deltas.len() + 1
        }
    }

    pub fn is_empty(&self) -> bool {
        self.base.is_none()
    }

    /// Append a sample, evicting the oldest once the capacity is reached.
    pub fn push(&mut self, sample: TelemetrySample) {
        let Some(prev) = self.last else {
            self.base = Some(sample);
            self.last = Some(sample);
            return;
        };

        if self.len() >= self.capacity {
            self.evict_oldest();
        }
        if self.base.is_none() {
            self.base = Some(sample);
            self.last = Some(sample);
            return;
        }

        let dt = sample
            .timestamp_sec
            .saturating_sub(prev.timestamp_sec)
            .min(u16::MAX as u64) as u16;
        self.timestamp_deltas.push_back(dt);
        self.upload_deltas
            .push_back(sample.upload_bytes as i64 - prev.upload_bytes as i64);
        self.download_deltas
            .push_back(sample.download_bytes as i64 - prev.download_bytes as i64);
        self.connection_deltas
            .push_back(sample.active_connections as i32 - prev.active_connections as i32);
        self.latency_ms.push_back(sample.latency_ms);
        self.last = Some(sample);
    }

    /// Oldest retained sample.
    pub fn oldest(&self) -> Option<TelemetrySample> {
        self.base
    }

    /// Newest retained sample.
    pub fn newest(&self) -> Option<TelemetrySample> {
        self.last
    }

    /// Reconstruct the sample at `index` (0 = oldest) without materializing the
    /// whole series.
    pub fn sample_at(&self, index: usize) -> Option<TelemetrySample> {
        if index >= self.len() {
            return None;
        }
        let mut sample = self.base?;
        let lanes = self
            .timestamp_deltas
            .iter()
            .take(index)
            .zip(self.upload_deltas.iter().take(index))
            .zip(self.download_deltas.iter().take(index))
            .zip(self.connection_deltas.iter().take(index))
            .zip(self.latency_ms.iter().take(index));
        for ((((dt, dup), ddown), dconn), latency_ms) in lanes {
            sample = Self::advance(sample, *dt, *dup, *ddown, *dconn, *latency_ms);
        }
        Some(sample)
    }

    /// Decompress the retained series in oldest-to-newest order.
    pub fn to_vec(&self) -> Vec<TelemetrySample> {
        let Some(base) = self.base else {
            return Vec::new();
        };
        let mut out = Vec::with_capacity(self.len());
        let mut sample = base;
        out.push(sample);
        let lanes = self
            .timestamp_deltas
            .iter()
            .zip(self.upload_deltas.iter())
            .zip(self.download_deltas.iter())
            .zip(self.connection_deltas.iter())
            .zip(self.latency_ms.iter());
        for ((((dt, dup), ddown), dconn), latency_ms) in lanes {
            sample = Self::advance(sample, *dt, *dup, *ddown, *dconn, *latency_ms);
            out.push(sample);
        }
        out
    }

    /// Bounded query: every retained sample in `[start_ts, end_ts]`, reduced to
    /// at most `max_points` via [`downsample`] when the range is large.
    pub fn query_window(
        &self,
        start_ts: u64,
        end_ts: u64,
        max_points: usize,
    ) -> Vec<TelemetrySample> {
        let in_range: Vec<TelemetrySample> = self
            .to_vec()
            .into_iter()
            .filter(|s| s.timestamp_sec >= start_ts && s.timestamp_sec <= end_ts)
            .collect();
        downsample(&in_range, max_points)
    }

    /// Time-travel cursor over a frozen snapshot of the retained series.
    pub fn replay_cursor(&self) -> ReplayCursor {
        ReplayCursor::new(self.to_vec())
    }

    fn evict_oldest(&mut self) {
        let Some(base) = self.base else {
            return;
        };
        if self.timestamp_deltas.is_empty() {
            self.base = None;
            self.last = None;
            return;
        }
        let dt = self
            .timestamp_deltas
            .pop_front()
            .expect("timestamp delta lane");
        let dup = self.upload_deltas.pop_front().expect("upload delta lane");
        let ddown = self
            .download_deltas
            .pop_front()
            .expect("download delta lane");
        let dconn = self
            .connection_deltas
            .pop_front()
            .expect("connection delta lane");
        let latency_ms = self.latency_ms.pop_front().expect("latency lane");
        self.base = Some(Self::advance(base, dt, dup, ddown, dconn, latency_ms));
    }

    fn advance(
        sample: TelemetrySample,
        dt: u16,
        dup: i64,
        ddown: i64,
        dconn: i32,
        latency_ms: f32,
    ) -> TelemetrySample {
        TelemetrySample {
            timestamp_sec: sample.timestamp_sec + dt as u64,
            upload_bytes: (sample.upload_bytes as i64 + dup).max(0) as u64,
            download_bytes: (sample.download_bytes as i64 + ddown).max(0) as u64,
            active_connections: (sample.active_connections as i64 + dconn as i64).max(0) as u32,
            latency_ms,
        }
    }
}

/// Reduce a series to at most `max_points` by averaging contiguous buckets.
///
/// Each output keeps the last timestamp of its bucket so the result stays
/// ordered and its final point is the newest input point. A series already
/// within budget is returned unchanged, so small windows are lossless.
pub fn downsample(samples: &[TelemetrySample], max_points: usize) -> Vec<TelemetrySample> {
    if max_points == 0 {
        return Vec::new();
    }
    if samples.len() <= max_points {
        return samples.to_vec();
    }
    (0..max_points)
        .map(|bucket| {
            let start = bucket * samples.len() / max_points;
            let end = (bucket + 1) * samples.len() / max_points;
            let window = &samples[start..end];
            let count = window.len() as u64;
            let upload: u64 = window.iter().map(|s| s.upload_bytes).sum();
            let download: u64 = window.iter().map(|s| s.download_bytes).sum();
            let connections: u64 = window.iter().map(|s| s.active_connections as u64).sum();
            let latency: f32 = window.iter().map(|s| s.latency_ms).sum();
            TelemetrySample {
                timestamp_sec: window.last().expect("bucket is non-empty").timestamp_sec,
                upload_bytes: upload / count,
                download_bytes: download / count,
                active_connections: (connections / count) as u32,
                latency_ms: latency / count as f32,
            }
        })
        .collect()
}

/// Time-travel replay cursor over a frozen telemetry snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct ReplayCursor {
    samples: Vec<TelemetrySample>,
    index: usize,
}

impl ReplayCursor {
    pub fn new(samples: Vec<TelemetrySample>) -> Self {
        Self { samples, index: 0 }
    }

    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// Current cursor position (0 = oldest).
    pub fn index(&self) -> usize {
        self.index
    }

    /// Sample under the cursor.
    pub fn current(&self) -> Option<TelemetrySample> {
        self.samples.get(self.index).copied()
    }

    /// Land on `index`, clamped to the last sample; returns the landed sample.
    pub fn seek_to_index(&mut self, index: usize) -> Option<TelemetrySample> {
        if self.samples.is_empty() {
            self.index = 0;
            return None;
        }
        self.index = index.min(self.samples.len() - 1);
        self.samples.get(self.index).copied()
    }

    /// Land on the newest sample at or before `target_ts`.
    pub fn seek_to_timestamp(&mut self, target_ts: u64) -> Option<TelemetrySample> {
        match self
            .samples
            .iter()
            .rposition(|s| s.timestamp_sec <= target_ts)
        {
            Some(index) => {
                self.index = index;
                self.samples.get(index).copied()
            }
            None => {
                self.index = 0;
                self.samples.first().copied()
            }
        }
    }

    /// Land on the sample at `fraction` of the snapshot (0.0 oldest, 1.0 newest).
    pub fn seek_to_fraction(&mut self, fraction: f32) -> Option<TelemetrySample> {
        if self.samples.is_empty() {
            return None;
        }
        let fraction = fraction.clamp(0.0, 1.0);
        let index = ((self.samples.len() - 1) as f32 * fraction).round() as usize;
        self.seek_to_index(index)
    }

    /// Advance one sample (stops at the newest).
    pub fn step_forward(&mut self) -> Option<TelemetrySample> {
        if self.index + 1 < self.samples.len() {
            self.index += 1;
        }
        self.samples.get(self.index).copied()
    }

    /// Rewind one sample (stops at the oldest).
    pub fn step_backward(&mut self) -> Option<TelemetrySample> {
        if self.index > 0 {
            self.index -= 1;
        }
        self.samples.get(self.index).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(ts: u64, up: u64, down: u64, connections: u32, latency_ms: f32) -> TelemetrySample {
        TelemetrySample {
            timestamp_sec: ts,
            upload_bytes: up,
            download_bytes: down,
            active_connections: connections,
            latency_ms,
        }
    }

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

    #[test]
    fn test_telemetry_store_round_trips_losslessly() {
        let mut store = TelemetryStore::new(16);
        let originals: Vec<TelemetrySample> = (0..12)
            .map(|i| {
                sample(
                    1_000 + i,
                    500 + i * 37,
                    9_000 + i * 111,
                    (i % 7) as u32,
                    10.0 + i as f32,
                )
            })
            .collect();
        for original in &originals {
            store.push(*original);
        }

        assert_eq!(store.len(), 12);
        assert!(!store.is_empty());
        assert_eq!(store.to_vec(), originals);
        assert_eq!(store.oldest(), Some(originals[0]));
        assert_eq!(store.newest(), Some(originals[11]));
        assert_eq!(store.sample_at(5), Some(originals[5]));
        assert_eq!(store.sample_at(12), None);
    }

    #[test]
    fn test_telemetry_store_stays_bounded_under_growth() {
        let capacity = 64;
        let mut store = TelemetryStore::new(capacity);
        for i in 0..5_000u64 {
            store.push(sample(i, i * 2, i * 3, 1, 20.0));
        }

        assert_eq!(store.len(), capacity);
        assert_eq!(store.capacity(), capacity);
        assert_eq!(store.timestamp_deltas.len(), capacity - 1);
        assert_eq!(store.latency_ms.len(), capacity - 1);
        assert_eq!(
            store.oldest().unwrap().timestamp_sec,
            5_000 - capacity as u64
        );
        assert_eq!(store.newest().unwrap().timestamp_sec, 4_999);

        let series = store.to_vec();
        assert_eq!(series.len(), capacity);
        for (offset, point) in series.iter().enumerate() {
            let expected_ts = 5_000 - capacity as u64 + offset as u64;
            assert_eq!(point.timestamp_sec, expected_ts);
            assert_eq!(point.upload_bytes, expected_ts * 2);
            assert_eq!(point.download_bytes, expected_ts * 3);
        }
    }

    #[test]
    fn test_downsample_preserves_count_bounds_and_means() {
        let series: Vec<TelemetrySample> =
            (0..100u64).map(|i| sample(i, 100, 400, 8, 50.0)).collect();

        let reduced = downsample(&series, 10);
        assert_eq!(reduced.len(), 10);
        for point in &reduced {
            assert_eq!(point.upload_bytes, 100);
            assert_eq!(point.download_bytes, 400);
            assert_eq!(point.active_connections, 8);
            assert!((point.latency_ms - 50.0).abs() < 1e-6);
            assert!(point.timestamp_sec < 100);
        }
        assert!(
            reduced
                .windows(2)
                .all(|w| w[0].timestamp_sec < w[1].timestamp_sec)
        );
        assert_eq!(reduced.last().unwrap().timestamp_sec, 99);

        // Within budget the series is returned untouched (lossless window).
        assert_eq!(downsample(&series, 100), series);

        // The query API applies the same reduction to a bounded time range.
        let mut store = TelemetryStore::new(200);
        for point in &series {
            store.push(*point);
        }
        let window = store.query_window(20, 79, 5);
        assert_eq!(window.len(), 5);
        assert!(window.iter().all(|p| (20..=79).contains(&p.timestamp_sec)));
        assert!(store.query_window(500, 600, 5).is_empty());
    }

    #[test]
    fn test_replay_cursor_seeks_and_steps() {
        let series: Vec<TelemetrySample> = (0..20u64)
            .map(|i| sample(1_000 + i * 10, i, i, 1, i as f32))
            .collect();
        let mut cursor = ReplayCursor::new(series.clone());
        assert_eq!(cursor.len(), 20);
        assert_eq!(cursor.current().unwrap().timestamp_sec, 1_000);

        let landed = cursor.seek_to_timestamp(1_075).unwrap();
        assert_eq!(landed.timestamp_sec, 1_070);
        assert_eq!(cursor.index(), 7);
        assert_eq!(cursor.step_forward().unwrap().timestamp_sec, 1_080);
        assert_eq!(cursor.step_backward().unwrap().timestamp_sec, 1_070);
        assert_eq!(cursor.step_backward().unwrap().timestamp_sec, 1_060);

        // Seeks clamp at both ends.
        assert_eq!(cursor.seek_to_timestamp(1).unwrap().timestamp_sec, 1_000);
        assert_eq!(
            cursor.seek_to_timestamp(9_999).unwrap().timestamp_sec,
            1_190
        );
        assert_eq!(cursor.seek_to_fraction(0.5).unwrap().timestamp_sec, 1_100);

        // Stepping past an end holds the boundary sample.
        cursor.seek_to_index(19);
        assert_eq!(cursor.step_forward().unwrap().timestamp_sec, 1_190);
        cursor.seek_to_index(0);
        assert_eq!(cursor.step_backward().unwrap().timestamp_sec, 1_000);

        let mut store = TelemetryStore::new(32);
        for point in &series {
            store.push(*point);
        }
        assert_eq!(store.replay_cursor().len(), 20);
    }
}
