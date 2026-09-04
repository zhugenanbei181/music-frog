use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A record representing the hit statistics for a specific traffic rule.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct RuleHitRecord {
    pub rule_raw: String,
    pub hit_count: u64,
    pub last_hit_secs: u64,
    pub total_payload_bytes: u64,
}

/// A counter and analyzer for tracking traffic rule hits and hotspot statistics.
#[derive(Debug, Default, Clone)]
pub struct RuleHitCounter {
    records: HashMap<String, RuleHitRecord>,
    total_hits: u64,
}

impl RuleHitCounter {
    /// Creates a new, empty `RuleHitCounter`.
    pub fn new() -> Self {
        Self {
            records: HashMap::new(),
            total_hits: 0,
        }
    }

    /// Records a hit for a specific rule, accumulating its payload bytes and updating the last hit timestamp.
    pub fn record_hit(&mut self, rule_raw: &str, payload_bytes: u64, timestamp_secs: u64) {
        self.total_hits += 1;
        let record = self
            .records
            .entry(rule_raw.to_string())
            .or_insert_with(|| RuleHitRecord {
                rule_raw: rule_raw.to_string(),
                hit_count: 0,
                last_hit_secs: 0,
                total_payload_bytes: 0,
            });

        record.hit_count += 1;
        record.total_payload_bytes += payload_bytes;
        if timestamp_secs > record.last_hit_secs {
            record.last_hit_secs = timestamp_secs;
        }
    }

    /// Returns the top rules sorted descending by hit count.
    /// If hit counts are equal, sorts alphabetically by rule_raw.
    pub fn top_rules_by_hits(&self, limit: usize) -> Vec<RuleHitRecord> {
        let mut sorted_records: Vec<_> = self.records.values().cloned().collect();
        sorted_records.sort_by(|a, b| {
            b.hit_count
                .cmp(&a.hit_count)
                .then_with(|| a.rule_raw.cmp(&b.rule_raw))
        });
        sorted_records.into_iter().take(limit).collect()
    }

    /// Returns the top rules sorted descending by total payload bytes.
    /// If traffic amounts are equal, sorts alphabetically by rule_raw.
    pub fn top_rules_by_traffic(&self, limit: usize) -> Vec<RuleHitRecord> {
        let mut sorted_records: Vec<_> = self.records.values().cloned().collect();
        sorted_records.sort_by(|a, b| {
            b.total_payload_bytes
                .cmp(&a.total_payload_bytes)
                .then_with(|| a.rule_raw.cmp(&b.rule_raw))
        });
        sorted_records.into_iter().take(limit).collect()
    }

    /// Returns the total number of hits recorded across all rules.
    pub fn total_hits(&self) -> u64 {
        self.total_hits
    }

    /// Clears all recorded statistics.
    pub fn clear(&mut self) {
        self.records.clear();
        self.total_hits = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_hit() {
        let mut counter = RuleHitCounter::new();
        counter.record_hit("rule1", 100, 10);
        counter.record_hit("rule1", 200, 20);

        assert_eq!(counter.total_hits(), 2);
        let top = counter.top_rules_by_hits(1);
        assert_eq!(top.len(), 1);
        assert_eq!(top[0].hit_count, 2);
        assert_eq!(top[0].total_payload_bytes, 300);
        assert_eq!(top[0].last_hit_secs, 20);
    }

    #[test]
    fn test_timestamp_update() {
        let mut counter = RuleHitCounter::new();
        counter.record_hit("rule1", 100, 20); // later time first
        counter.record_hit("rule1", 200, 10); // earlier time second

        let top = counter.top_rules_by_hits(1);
        assert_eq!(top[0].last_hit_secs, 20); // should retain the higher timestamp
    }

    #[test]
    fn test_top_rules_by_hits() {
        let mut counter = RuleHitCounter::new();
        counter.record_hit("rule1", 100, 10);
        counter.record_hit("rule2", 200, 20);
        counter.record_hit("rule2", 100, 25);
        counter.record_hit("rule3", 50, 30);
        counter.record_hit("rule3", 50, 35);
        counter.record_hit("rule3", 50, 40);

        let top = counter.top_rules_by_hits(2);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].rule_raw, "rule3"); // 3 hits
        assert_eq!(top[1].rule_raw, "rule2"); // 2 hits
    }

    #[test]
    fn test_top_rules_by_traffic() {
        let mut counter = RuleHitCounter::new();
        counter.record_hit("rule1", 1000, 10); // 1000 bytes
        counter.record_hit("rule2", 200, 20);
        counter.record_hit("rule2", 300, 25); // 500 bytes
        counter.record_hit("rule3", 50, 30);
        counter.record_hit("rule3", 50, 35);
        counter.record_hit("rule3", 50, 40); // 150 bytes

        let top = counter.top_rules_by_traffic(2);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].rule_raw, "rule1"); // 1000 bytes
        assert_eq!(top[1].rule_raw, "rule2"); // 500 bytes
    }

    #[test]
    fn test_clear() {
        let mut counter = RuleHitCounter::new();
        counter.record_hit("rule1", 100, 10);
        assert_eq!(counter.total_hits(), 1);

        counter.clear();
        assert_eq!(counter.total_hits(), 0);
        assert_eq!(counter.top_rules_by_hits(10).len(), 0);
        assert_eq!(counter.top_rules_by_traffic(10).len(), 0);
    }
}
