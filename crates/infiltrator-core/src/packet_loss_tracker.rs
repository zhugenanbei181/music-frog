use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProbeSample {
    pub timestamp_secs: u64,
    pub rtt_ms: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeReliabilityReport {
    pub total_probes: usize,
    pub lost_probes: usize,
    pub loss_rate_percent: f64,
    pub avg_rtt_ms: Option<f64>,
    pub jitter_ms: Option<f64>,
    pub reliability_score: u8,
}

#[derive(Debug, Clone)]
pub struct PacketLossTracker {
    window_size: usize,
    samples: VecDeque<ProbeSample>,
}

impl PacketLossTracker {
    pub fn new(window_size: usize) -> Self {
        Self {
            window_size,
            samples: VecDeque::with_capacity(window_size),
        }
    }

    pub fn record_probe(&mut self, rtt_ms: Option<u32>, timestamp_secs: u64) {
        if self.samples.len() == self.window_size {
            self.samples.pop_front();
        }
        self.samples.push_back(ProbeSample {
            timestamp_secs,
            rtt_ms,
        });
    }

    pub fn compute_report(&self) -> NodeReliabilityReport {
        let total_probes = self.samples.len();
        if total_probes == 0 {
            return NodeReliabilityReport {
                total_probes: 0,
                lost_probes: 0,
                loss_rate_percent: 0.0,
                avg_rtt_ms: None,
                jitter_ms: None,
                reliability_score: 100,
            };
        }

        let mut lost_probes = 0;
        let mut rtt_sum = 0.0;
        let mut successful_probes = 0;
        let mut prev_rtt = None;
        let mut jitter = 0.0;

        for sample in &self.samples {
            if let Some(rtt) = sample.rtt_ms {
                let rtt_f64 = rtt as f64;
                rtt_sum += rtt_f64;
                successful_probes += 1;

                if let Some(prev) = prev_rtt {
                    let diff = if rtt_f64 > prev { rtt_f64 - prev } else { prev - rtt_f64 };
                    jitter = jitter + (diff - jitter) / 16.0;
                }
                prev_rtt = Some(rtt_f64);
            } else {
                lost_probes += 1;
            }
        }

        let loss_rate_percent = (lost_probes as f64 / total_probes as f64) * 100.0;
        let avg_rtt_ms = if successful_probes > 0 {
            Some(rtt_sum / successful_probes as f64)
        } else {
            None
        };
        
        let jitter_ms = if successful_probes > 1 {
            Some(jitter)
        } else {
            None
        };

        // Calculate reliability score (0-100)
        let mut score = 100.0;
        // Deduct points for loss rate (e.g. 2 points per 1% loss)
        score -= loss_rate_percent * 2.0;
        
        // Deduct points for average latency (e.g. 1 point per 10ms over 50ms)
        if let Some(avg) = avg_rtt_ms {
            if avg > 50.0 {
                score -= (avg - 50.0) / 10.0;
            }
        }
        
        // Deduct points for jitter (e.g. 1 point per 5ms)
        if let Some(j) = jitter_ms {
            score -= j / 5.0;
        }

        let reliability_score = score.clamp(0.0, 100.0).round() as u8;

        NodeReliabilityReport {
            total_probes,
            lost_probes,
            loss_rate_percent,
            avg_rtt_ms,
            jitter_ms,
            reliability_score,
        }
    }

    pub fn is_healthy(&self, max_loss_rate_percent: f64) -> bool {
        let report = self.compute_report();
        report.loss_rate_percent <= max_loss_rate_percent
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_tracker() {
        let tracker = PacketLossTracker::new(10);
        let report = tracker.compute_report();
        assert_eq!(report.total_probes, 0);
        assert_eq!(report.loss_rate_percent, 0.0);
        assert_eq!(report.reliability_score, 100);
        assert!(tracker.is_healthy(10.0));
    }

    #[test]
    fn test_zero_loss() {
        let mut tracker = PacketLossTracker::new(10);
        for i in 0..10 {
            tracker.record_probe(Some(50), i as u64);
        }
        let report = tracker.compute_report();
        assert_eq!(report.total_probes, 10);
        assert_eq!(report.lost_probes, 0);
        assert_eq!(report.loss_rate_percent, 0.0);
        assert_eq!(report.avg_rtt_ms, Some(50.0));
        assert_eq!(report.jitter_ms, Some(0.0));
        assert_eq!(report.reliability_score, 100);
        assert!(tracker.is_healthy(5.0));
    }

    #[test]
    fn test_total_loss() {
        let mut tracker = PacketLossTracker::new(10);
        for i in 0..10 {
            tracker.record_probe(None, i as u64);
        }
        let report = tracker.compute_report();
        assert_eq!(report.total_probes, 10);
        assert_eq!(report.lost_probes, 10);
        assert_eq!(report.loss_rate_percent, 100.0);
        assert_eq!(report.avg_rtt_ms, None);
        assert_eq!(report.jitter_ms, None);
        assert_eq!(report.reliability_score, 0);
        assert!(!tracker.is_healthy(99.0));
    }

    #[test]
    fn test_mixed_loss_and_sliding_window() {
        let mut tracker = PacketLossTracker::new(5);
        // Fill window with success
        for i in 0..5 {
            tracker.record_probe(Some(40), i);
        }
        // Overwrite two oldest with losses
        tracker.record_probe(None, 5);
        tracker.record_probe(None, 6);

        let report = tracker.compute_report();
        assert_eq!(report.total_probes, 5);
        assert_eq!(report.lost_probes, 2);
        assert_eq!(report.loss_rate_percent, 40.0);
        assert_eq!(report.avg_rtt_ms, Some(40.0));
        assert_eq!(report.reliability_score, 20); // 100 - (40 * 2) = 20
        assert!(!tracker.is_healthy(30.0));
        assert!(tracker.is_healthy(45.0));
    }

    #[test]
    fn test_jitter_calculation() {
        let mut tracker = PacketLossTracker::new(5);
        tracker.record_probe(Some(40), 1);
        tracker.record_probe(Some(60), 2);
        
        let report = tracker.compute_report();
        // first diff is |60-40| = 20
        // J = 0 + (20 - 0) / 16 = 1.25
        assert_eq!(report.jitter_ms, Some(1.25));

        tracker.record_probe(Some(60), 3);
        let report2 = tracker.compute_report();
        // second diff is |60-60| = 0
        // J = 1.25 + (0 - 1.25) / 16 = 1.25 - 0.078125 = 1.171875
        assert_eq!(report2.jitter_ms, Some(1.171875));
    }
}
