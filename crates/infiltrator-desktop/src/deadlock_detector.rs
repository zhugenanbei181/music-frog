use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct LockContentionEvent {
    pub lock_name: String,
    pub thread_id: String,
    pub wait_duration_ms: u64,
    pub acquired: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct DeadlockReport {
    pub suspected_deadlocks: Vec<String>,
    pub max_wait_ms: u64,
    pub total_contentions: usize,
}

pub struct DeadlockDetector {
    contention_threshold_ms: u64,
    events: Vec<LockContentionEvent>,
}

impl DeadlockDetector {
    pub fn new(contention_threshold_ms: u64) -> Self {
        Self {
            contention_threshold_ms,
            events: Vec::new(),
        }
    }

    pub fn record_acquisition(
        &mut self,
        lock_name: &str,
        thread_id: &str,
        wait_duration_ms: u64,
    ) {
        self.events.push(LockContentionEvent {
            lock_name: lock_name.to_string(),
            thread_id: thread_id.to_string(),
            wait_duration_ms,
            acquired: true,
        });
    }

    pub fn generate_report(&self) -> DeadlockReport {
        let mut suspected_deadlocks = Vec::new();
        let mut max_wait_ms = 0;
        let mut total_contentions = 0;

        for event in &self.events {
            if event.wait_duration_ms >= self.contention_threshold_ms {
                total_contentions += 1;
            }
            if event.wait_duration_ms > max_wait_ms {
                max_wait_ms = event.wait_duration_ms;
            }
            if (event.wait_duration_ms > self.contention_threshold_ms * 2 || !event.acquired)
                && !suspected_deadlocks.contains(&event.lock_name) {
                    suspected_deadlocks.push(event.lock_name.clone());
                }
        }

        DeadlockReport {
            suspected_deadlocks,
            max_wait_ms,
            total_contentions,
        }
    }

    pub fn has_active_contention(&self) -> bool {
        self.events
            .iter()
            .any(|e| e.wait_duration_ms >= self.contention_threshold_ms)
    }

    pub fn clear(&mut self) {
        self.events.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_benign_short_lock_acquisitions() {
        let mut detector = DeadlockDetector::new(100);
        detector.record_acquisition("lock_a", "thread_1", 10);
        detector.record_acquisition("lock_b", "thread_2", 20);

        let report = detector.generate_report();
        assert_eq!(report.total_contentions, 0);
        assert_eq!(report.max_wait_ms, 20);
        assert!(report.suspected_deadlocks.is_empty());
        assert!(!detector.has_active_contention());
    }

    #[test]
    fn test_threshold_exceeding_contention_event_logging() {
        let mut detector = DeadlockDetector::new(100);
        detector.record_acquisition("lock_a", "thread_1", 150);

        let report = detector.generate_report();
        assert_eq!(report.total_contentions, 1);
        assert_eq!(report.max_wait_ms, 150);
        assert!(report.suspected_deadlocks.is_empty());
        assert!(detector.has_active_contention());
    }

    #[test]
    fn test_report_generation_and_deadlock_suspicion_identification() {
        let mut detector = DeadlockDetector::new(100);
        detector.record_acquisition("lock_a", "thread_1", 250);

        let report = detector.generate_report();
        assert_eq!(report.total_contentions, 1);
        assert_eq!(report.max_wait_ms, 250);
        assert_eq!(report.suspected_deadlocks, vec!["lock_a".to_string()]);
        assert!(detector.has_active_contention());
    }

    #[test]
    fn test_clear_and_reset_behavior() {
        let mut detector = DeadlockDetector::new(100);
        detector.record_acquisition("lock_a", "thread_1", 250);
        assert!(detector.has_active_contention());

        detector.clear();
        assert!(!detector.has_active_contention());

        let report = detector.generate_report();
        assert_eq!(report.total_contentions, 0);
        assert_eq!(report.max_wait_ms, 0);
        assert!(report.suspected_deadlocks.is_empty());
    }
}
