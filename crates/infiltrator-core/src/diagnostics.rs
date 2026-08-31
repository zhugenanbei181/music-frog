use std::time::Instant;

/// Snapshot of connection transfer rates at a given time.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ConnectionRateSnapshot {
    pub up_speed: u64,
    pub down_speed: u64,
    pub total_up: u64,
    pub total_down: u64,
    pub peak_up_speed: u64,
    pub peak_down_speed: u64,
}

/// Tracks per-connection upload/download byte counters over time and computes instantaneous rates.
pub struct ConnectionRateTracker {
    total_up: u64,
    total_down: u64,
    last_snapshot_time: Instant,
    last_snapshot_up: u64,
    last_snapshot_down: u64,
    peak_up_speed: u64,
    peak_down_speed: u64,
}

impl Default for ConnectionRateTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl ConnectionRateTracker {
    /// Creates a new `ConnectionRateTracker`.
    pub fn new() -> Self {
        Self::new_with_time(Instant::now())
    }
    /// Creates a new `ConnectionRateTracker` with a specific start time.
    pub fn new_with_time(now: Instant) -> Self {
        Self {
            total_up: 0,
            total_down: 0,
            last_snapshot_time: now,
            last_snapshot_up: 0,
            last_snapshot_down: 0,
            peak_up_speed: 0,
            peak_down_speed: 0,
        }
    }

    /// Add bytes to the total upload counter.
    pub fn add_up(&mut self, bytes: u64) {
        self.total_up += bytes;
    }

    /// Add bytes to the total download counter.
    pub fn add_down(&mut self, bytes: u64) {
        self.total_down += bytes;
    }

    /// Takes a snapshot of the current rates and updates internal state using `Instant::now()`.
    pub fn snapshot(&mut self) -> ConnectionRateSnapshot {
        self.snapshot_with_time(Instant::now())
    }

    /// Takes a snapshot of the current rates and updates internal state using the provided time.
    pub fn snapshot_with_time(&mut self, now: Instant) -> ConnectionRateSnapshot {
        let elapsed = now
            .saturating_duration_since(self.last_snapshot_time)
            .as_secs_f64();

        let mut up_speed = 0;
        let mut down_speed = 0;

        if elapsed > 0.0 {
            let up_diff = self.total_up.saturating_sub(self.last_snapshot_up);
            let down_diff = self.total_down.saturating_sub(self.last_snapshot_down);

            up_speed = (up_diff as f64 / elapsed) as u64;
            down_speed = (down_diff as f64 / elapsed) as u64;
        }

        self.peak_up_speed = self.peak_up_speed.max(up_speed);
        self.peak_down_speed = self.peak_down_speed.max(down_speed);

        self.last_snapshot_time = now;
        self.last_snapshot_up = self.total_up;
        self.last_snapshot_down = self.total_down;

        ConnectionRateSnapshot {
            up_speed,
            down_speed,
            total_up: self.total_up,
            total_down: self.total_down,
            peak_up_speed: self.peak_up_speed,
            peak_down_speed: self.peak_down_speed,
        }
    }
}

/// Statistics calculated from a series of latency measurements.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct JitterStats {
    pub mean_latency_ms: f64,
    pub jitter_ms: f64,
    pub std_dev_ms: f64,
    pub loss_rate_percent: f64,
    pub sample_count: usize,
}

/// Calculates latency jitter, standard deviation, and packet loss rate.
pub struct JitterCalculator {
    latencies: Vec<f64>,
    failures: usize,
    total_attempts: usize,
}

impl Default for JitterCalculator {
    fn default() -> Self {
        Self::new()
    }
}

impl JitterCalculator {
    /// Creates a new `JitterCalculator`.
    pub fn new() -> Self {
        Self {
            latencies: Vec::new(),
            failures: 0,
            total_attempts: 0,
        }
    }

    /// Records a successful measurement with the given latency.
    pub fn record_success(&mut self, latency_ms: f64) {
        self.latencies.push(latency_ms);
        self.total_attempts += 1;
    }

    /// Records a failed or timed-out measurement.
    pub fn record_failure(&mut self) {
        self.failures += 1;
        self.total_attempts += 1;
    }

    /// Computes and returns the jitter statistics based on recorded data.
    pub fn calculate(&self) -> JitterStats {
        let sample_count = self.latencies.len();
        let loss_rate_percent = if self.total_attempts > 0 {
            (self.failures as f64 / self.total_attempts as f64) * 100.0
        } else {
            0.0
        };

        if sample_count == 0 {
            return JitterStats {
                mean_latency_ms: 0.0,
                jitter_ms: 0.0,
                std_dev_ms: 0.0,
                loss_rate_percent,
                sample_count: self.total_attempts,
            };
        }

        let mean_latency_ms = self.latencies.iter().sum::<f64>() / sample_count as f64;

        let mut jitter_ms = 0.0;
        if sample_count > 1 {
            let mut sum_diff = 0.0;
            for i in 1..sample_count {
                sum_diff += (self.latencies[i] - self.latencies[i - 1]).abs();
            }
            jitter_ms = sum_diff / (sample_count - 1) as f64;
        }

        let mut std_dev_ms = 0.0;
        if sample_count > 1 {
            let variance = self
                .latencies
                .iter()
                .map(|&x| (x - mean_latency_ms).powi(2))
                .sum::<f64>()
                / (sample_count - 1) as f64;
            std_dev_ms = variance.sqrt();
        }

        JitterStats {
            mean_latency_ms,
            jitter_ms,
            std_dev_ms,
            loss_rate_percent,
            sample_count: self.total_attempts,
        }
    }
}

/// Report containing metadata about an outbound IP address.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OutboundIpReport {
    pub ip: String,
    pub country: Option<String>,
    pub city: Option<String>,
    pub asn: Option<u32>,
    pub isp: Option<String>,
    pub fraud_score: Option<u8>,
}

/// Tracks DNS queries, cache hits, fake-ip pool capacity and computes metrics.
pub struct DnsMetricsTracker {
    queries: usize,
    cache_hits: usize,
    fake_ip_capacity: usize,
    response_times: Vec<f64>,
}

impl DnsMetricsTracker {
    /// Creates a new `DnsMetricsTracker`.
    pub fn new(fake_ip_capacity: usize) -> Self {
        Self {
            queries: 0,
            cache_hits: 0,
            fake_ip_capacity,
            response_times: Vec::new(),
        }
    }

    /// Records a DNS query result.
    pub fn record_query(&mut self, is_hit: bool, response_time_ms: f64) {
        self.queries += 1;
        if is_hit {
            self.cache_hits += 1;
        }
        self.response_times.push(response_time_ms);
    }

    /// Returns the cache hit ratio (0.0 to 1.0).
    pub fn hit_ratio(&self) -> f64 {
        if self.queries == 0 {
            0.0
        } else {
            self.cache_hits as f64 / self.queries as f64
        }
    }

    /// Returns the average DNS response time in milliseconds.
    pub fn average_response_time_ms(&self) -> f64 {
        if self.response_times.is_empty() {
            0.0
        } else {
            self.response_times.iter().sum::<f64>() / self.response_times.len() as f64
        }
    }

    /// Returns the fake IP pool capacity.
    pub fn fake_ip_capacity(&self) -> usize {
        self.fake_ip_capacity
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_connection_rate_tracker() {
        let start = Instant::now();
        let mut tracker = ConnectionRateTracker::new_with_time(start);

        tracker.add_up(1000);
        tracker.add_down(2000);

        let t1 = start + Duration::from_secs(1);
        let snap1 = tracker.snapshot_with_time(t1);

        assert_eq!(snap1.total_up, 1000);
        assert_eq!(snap1.total_down, 2000);
        assert_eq!(snap1.up_speed, 1000);
        assert_eq!(snap1.down_speed, 2000);
        assert_eq!(snap1.peak_up_speed, 1000);
        assert_eq!(snap1.peak_down_speed, 2000);

        tracker.add_up(500);
        tracker.add_down(4000);

        let t2 = t1 + Duration::from_secs_f64(0.5);
        let snap2 = tracker.snapshot_with_time(t2);

        assert_eq!(snap2.total_up, 1500);
        assert_eq!(snap2.total_down, 6000);
        assert_eq!(snap2.up_speed, 1000); // 500 / 0.5
        assert_eq!(snap2.down_speed, 8000); // 4000 / 0.5
        assert_eq!(snap2.peak_up_speed, 1000);
        assert_eq!(snap2.peak_down_speed, 8000);
    }

    #[test]
    fn test_jitter_calculator() {
        let mut calc = JitterCalculator::new();
        // latencies [100, 110, 105, 120]
        calc.record_success(100.0);
        calc.record_success(110.0);
        calc.record_success(105.0);
        calc.record_success(120.0);

        let stats = calc.calculate();

        assert_eq!(stats.sample_count, 4);
        assert_eq!(stats.loss_rate_percent, 0.0);
        assert_eq!(stats.mean_latency_ms, 108.75); // (100+110+105+120)/4

        // differences: |110-100|=10, |105-110|=5, |120-105|=15
        // MAD jitter: (10 + 5 + 15) / 3 = 10
        assert_eq!(stats.jitter_ms, 10.0);

        // Variance:
        // (100-108.75)^2 = 76.5625
        // (110-108.75)^2 = 1.5625
        // (105-108.75)^2 = 14.0625
        // (120-108.75)^2 = 126.5625
        // Sum = 218.75
        // std_dev = sqrt(218.75 / 3) ≈ 8.53912
        assert!((stats.std_dev_ms - 8.53912).abs() < 0.001);
    }

    #[test]
    fn test_jitter_calculator_with_loss() {
        let mut calc = JitterCalculator::new();
        calc.record_success(50.0);
        calc.record_failure();
        calc.record_success(60.0);
        calc.record_failure();
        calc.record_failure();

        let stats = calc.calculate();
        assert_eq!(stats.sample_count, 5);
        assert_eq!(stats.loss_rate_percent, 60.0); // 3 failures / 5 total attempts
        assert_eq!(stats.mean_latency_ms, 55.0);
        assert_eq!(stats.jitter_ms, 10.0);
    }

    #[test]
    fn test_dns_metrics_tracker() {
        let mut tracker = DnsMetricsTracker::new(1000);
        assert_eq!(tracker.fake_ip_capacity(), 1000);

        tracker.record_query(true, 10.0);
        tracker.record_query(false, 50.0);
        tracker.record_query(true, 5.0);
        tracker.record_query(false, 35.0);

        assert_eq!(tracker.hit_ratio(), 0.5); // 2 hits out of 4
        assert_eq!(tracker.average_response_time_ms(), 25.0); // (10+50+5+35)/4 = 100/4
    }
}
