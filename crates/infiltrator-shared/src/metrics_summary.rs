use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct MetricsSnapshot {
    pub upload_speed_bps: u64,
    pub download_speed_bps: u64,
    pub active_connections: usize,
    pub memory_usage_bytes: u64,
    pub uptime_seconds: u64,
    pub error_count: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct RollingSpeedStats {
    pub avg_upload_bps: u64,
    pub avg_download_bps: u64,
    pub peak_upload_bps: u64,
    pub peak_download_bps: u64,
}

pub struct MetricsAggregator {
    window_size: usize,
    samples: VecDeque<(u64, u64)>,
    peak_upload_bps: u64,
    peak_download_bps: u64,
}

impl MetricsAggregator {
    pub fn new(window_size: usize) -> Self {
        Self {
            window_size: window_size.max(1),
            samples: VecDeque::with_capacity(window_size),
            peak_upload_bps: 0,
            peak_download_bps: 0,
        }
    }

    pub fn record_sample(&mut self, upload_bps: u64, download_bps: u64) {
        if self.samples.len() == self.window_size {
            self.samples.pop_front();
        }
        self.samples.push_back((upload_bps, download_bps));
        
        self.peak_upload_bps = self.peak_upload_bps.max(upload_bps);
        self.peak_download_bps = self.peak_download_bps.max(download_bps);
    }

    pub fn compute_stats(&self) -> RollingSpeedStats {
        let count = self.samples.len();
        if count == 0 {
            return RollingSpeedStats {
                avg_upload_bps: 0,
                avg_download_bps: 0,
                peak_upload_bps: self.peak_upload_bps,
                peak_download_bps: self.peak_download_bps,
            };
        }

        let mut total_upload = 0;
        let mut total_download = 0;

        for &(up, down) in &self.samples {
            total_upload += up;
            total_download += down;
        }

        RollingSpeedStats {
            avg_upload_bps: total_upload / count as u64,
            avg_download_bps: total_download / count as u64,
            peak_upload_bps: self.peak_upload_bps,
            peak_download_bps: self.peak_download_bps,
        }
    }

    pub fn format_human_bandwidth(bytes_per_sec: u64) -> String {
        let kb = 1024_f64;
        let mb = kb * 1024_f64;
        let gb = mb * 1024_f64;

        let bps = bytes_per_sec as f64;

        if bps >= gb {
            format!("{:.1} GB/s", bps / gb)
        } else if bps >= mb {
            format!("{:.1} MB/s", bps / mb)
        } else if bps >= kb {
            format!("{:.1} KB/s", bps / kb)
        } else {
            format!("{} B/s", bytes_per_sec)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rolling_average_window_calculation() {
        let mut agg = MetricsAggregator::new(3);
        
        let stats = agg.compute_stats();
        assert_eq!(stats.avg_upload_bps, 0);
        assert_eq!(stats.avg_download_bps, 0);

        agg.record_sample(10, 20);
        let stats = agg.compute_stats();
        assert_eq!(stats.avg_upload_bps, 10);
        assert_eq!(stats.avg_download_bps, 20);

        agg.record_sample(20, 30);
        let stats = agg.compute_stats();
        assert_eq!(stats.avg_upload_bps, 15); // (10+20)/2
        assert_eq!(stats.avg_download_bps, 25); // (20+30)/2

        agg.record_sample(30, 40);
        let stats = agg.compute_stats();
        assert_eq!(stats.avg_upload_bps, 20); // (10+20+30)/3
        assert_eq!(stats.avg_download_bps, 30); // (20+30+40)/3

        // Window pushes old sample out
        agg.record_sample(40, 50);
        let stats = agg.compute_stats();
        assert_eq!(stats.avg_upload_bps, 30); // (20+30+40)/3
        assert_eq!(stats.avg_download_bps, 40); // (30+40+50)/3
    }

    #[test]
    fn test_peak_speed_retention() {
        let mut agg = MetricsAggregator::new(2);
        
        agg.record_sample(100, 200);
        agg.record_sample(50, 300);
        
        let stats = agg.compute_stats();
        assert_eq!(stats.peak_upload_bps, 100);
        assert_eq!(stats.peak_download_bps, 300);

        agg.record_sample(10, 20); // pushes out old samples, but peak remains
        let stats = agg.compute_stats();
        assert_eq!(stats.peak_upload_bps, 100);
        assert_eq!(stats.peak_download_bps, 300);
    }

    #[test]
    fn test_format_human_bandwidth() {
        assert_eq!(MetricsAggregator::format_human_bandwidth(500), "500 B/s");
        assert_eq!(MetricsAggregator::format_human_bandwidth(1024), "1.0 KB/s");
        assert_eq!(MetricsAggregator::format_human_bandwidth(1536), "1.5 KB/s");
        assert_eq!(MetricsAggregator::format_human_bandwidth(1048576), "1.0 MB/s");
        assert_eq!(MetricsAggregator::format_human_bandwidth(12992276), "12.4 MB/s");
        assert_eq!(MetricsAggregator::format_human_bandwidth(1073741824), "1.0 GB/s");
    }

    #[test]
    fn test_serialization_deserialization_roundtrip() {
        let snapshot = MetricsSnapshot {
            upload_speed_bps: 100,
            download_speed_bps: 200,
            active_connections: 5,
            memory_usage_bytes: 1024,
            uptime_seconds: 60,
            error_count: 1,
        };

        let serialized = serde_json::to_string(&snapshot).unwrap();
        let deserialized: MetricsSnapshot = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(snapshot, deserialized);

        let stats = RollingSpeedStats {
            avg_upload_bps: 50,
            avg_download_bps: 100,
            peak_upload_bps: 150,
            peak_download_bps: 200,
        };

        let serialized = serde_json::to_string(&stats).unwrap();
        let deserialized: RollingSpeedStats = serde_json::from_str(&serialized).unwrap();

        assert_eq!(stats, deserialized);
    }
}
