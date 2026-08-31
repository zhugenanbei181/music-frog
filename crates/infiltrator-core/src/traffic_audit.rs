use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub enum AuditRouteType {
    Proxied,
    DirectBypass,
    Reject,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct AppTrafficRecord {
    pub process_name: String,
    pub upload_bytes: u64,
    pub download_bytes: u64,
    pub packets_count: u64,
    pub route_type: AuditRouteType,
}

impl AppTrafficRecord {
    pub fn total_bytes(&self) -> u64 {
        self.upload_bytes + self.download_bytes
    }
}

#[derive(Debug, Clone)]
pub struct TrafficAuditAggregator {
    // Key: (Process Name, Route Type)
    records: HashMap<(String, AuditRouteType), AppTrafficRecord>,
}

impl TrafficAuditAggregator {
    pub fn new() -> Self {
        Self {
            records: HashMap::new(),
        }
    }

    pub fn record_flow(
        &mut self,
        process_name: &str,
        up_bytes: u64,
        down_bytes: u64,
        packets: u64,
        route: AuditRouteType,
    ) {
        let key = (process_name.to_string(), route.clone());
        let record = self.records.entry(key).or_insert(AppTrafficRecord {
            process_name: process_name.to_string(),
            upload_bytes: 0,
            download_bytes: 0,
            packets_count: 0,
            route_type: route,
        });

        record.upload_bytes += up_bytes;
        record.download_bytes += down_bytes;
        record.packets_count += packets;
    }

    pub fn total_proxied_bytes(&self) -> u64 {
        self.records
            .values()
            .filter(|r| r.route_type == AuditRouteType::Proxied)
            .map(|r| r.total_bytes())
            .sum()
    }

    pub fn total_direct_bytes(&self) -> u64 {
        self.records
            .values()
            .filter(|r| r.route_type == AuditRouteType::DirectBypass)
            .map(|r| r.total_bytes())
            .sum()
    }

    pub fn direct_bypass_ratio(&self) -> f64 {
        let direct = self.total_direct_bytes() as f64;
        let proxied = self.total_proxied_bytes() as f64;
        let total = direct + proxied;
        if total == 0.0 { 0.0 } else { direct / total }
    }

    pub fn top_processes_by_traffic(&self, limit: usize) -> Vec<AppTrafficRecord> {
        let mut sorted_records: Vec<_> = self.records.values().cloned().collect();
        // Sort in descending order
        sorted_records.sort_by_key(|record| std::cmp::Reverse(record.total_bytes()));
        sorted_records.into_iter().take(limit).collect()
    }

    pub fn clear(&mut self) {
        self.records.clear();
    }
}

impl Default for TrafficAuditAggregator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_flow_and_aggregation() {
        let mut aggregator = TrafficAuditAggregator::new();
        aggregator.record_flow("chrome", 100, 200, 10, AuditRouteType::Proxied);
        aggregator.record_flow("chrome", 50, 50, 5, AuditRouteType::Proxied);

        let records = aggregator.top_processes_by_traffic(10);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].upload_bytes, 150);
        assert_eq!(records[0].download_bytes, 250);
        assert_eq!(records[0].packets_count, 15);
    }

    #[test]
    fn test_total_proxied_vs_direct() {
        let mut aggregator = TrafficAuditAggregator::new();
        aggregator.record_flow("app1", 1000, 1000, 20, AuditRouteType::Proxied);
        aggregator.record_flow("app2", 500, 500, 10, AuditRouteType::DirectBypass);
        aggregator.record_flow("app3", 100, 100, 2, AuditRouteType::Reject);

        assert_eq!(aggregator.total_proxied_bytes(), 2000);
        assert_eq!(aggregator.total_direct_bytes(), 1000);
    }

    #[test]
    fn test_bypass_ratio() {
        let mut aggregator = TrafficAuditAggregator::new();
        assert_eq!(aggregator.direct_bypass_ratio(), 0.0);

        aggregator.record_flow("app1", 50, 50, 2, AuditRouteType::Proxied);
        assert_eq!(aggregator.direct_bypass_ratio(), 0.0); // 100% proxied, 0% direct

        aggregator.record_flow("app2", 50, 50, 2, AuditRouteType::DirectBypass);
        assert_eq!(aggregator.direct_bypass_ratio(), 0.5); // 50% proxied, 50% direct

        aggregator.clear();
        aggregator.record_flow("app3", 100, 100, 2, AuditRouteType::DirectBypass);
        assert_eq!(aggregator.direct_bypass_ratio(), 1.0); // 100% direct
    }

    #[test]
    fn test_top_processes_by_traffic() {
        let mut aggregator = TrafficAuditAggregator::new();
        aggregator.record_flow("low_traffic", 10, 10, 1, AuditRouteType::Proxied);
        aggregator.record_flow("high_traffic", 1000, 1000, 10, AuditRouteType::DirectBypass);
        aggregator.record_flow("med_traffic", 100, 100, 5, AuditRouteType::Proxied);

        let top = aggregator.top_processes_by_traffic(2);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].process_name, "high_traffic");
        assert_eq!(top[1].process_name, "med_traffic");
    }
}
