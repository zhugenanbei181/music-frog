//! Shared contract for traffic and closed connection history (SLINT-007).
//!
//! Owns the canonical schemas for:
//! - Rolling 7-day hourly traffic usage buckets and aggregations
//! - Bar chart representations for today (24 hourly bars) and current week (daily bars)
//! - Closed connection history records and query models

use serde::{Deserialize, Serialize};

/// One hourly traffic bucket recorded by the background history collector.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrafficUsageBucket {
    /// Local calendar date in YYYY-MM-DD format.
    pub day: String,
    /// Hour of the day (0..23).
    pub hour: u8,
    pub download_bytes: u64,
    pub upload_bytes: u64,
}

/// Normalized bar data for traffic history bar charts.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TrafficBarSnapshot {
    pub label: String,
    pub detail_label: String,
    pub download_bytes: u64,
    pub upload_bytes: u64,
    pub download_fraction: f32,
    pub upload_fraction: f32,
}

/// Complete traffic history snapshot projected to UI surfaces.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TrafficHistorySnapshot {
    pub today_download_bytes: u64,
    pub today_upload_bytes: u64,
    pub week_download_bytes: u64,
    pub week_upload_bytes: u64,
    pub week_average_download_bytes: u64,
    pub week_average_upload_bytes: u64,
    pub today_has_data: bool,
    pub week_has_data: bool,
    pub today_bars: Vec<TrafficBarSnapshot>,
    pub week_bars: Vec<TrafficBarSnapshot>,
    pub buckets: Vec<TrafficUsageBucket>,
}

/// One closed connection telemetry record.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClosedConnectionSnapshot {
    pub id: String,
    pub destination: String,
    pub duration_ms: u64,
    pub upload_bytes: u64,
    pub download_bytes: u64,
    pub rule: String,
    pub outbound: String,
    pub closed_at: String,
}

/// Collection of recent closed connections.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClosedConnectionsSnapshot {
    pub records: Vec<ClosedConnectionSnapshot>,
    pub total_closed: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traffic_history_defaults_to_honest_empty() {
        let snapshot = TrafficHistorySnapshot::default();
        assert!(!snapshot.today_has_data);
        assert!(!snapshot.week_has_data);
        assert_eq!(snapshot.today_download_bytes, 0);
        assert_eq!(snapshot.today_upload_bytes, 0);
        assert!(snapshot.today_bars.is_empty());
        assert!(snapshot.week_bars.is_empty());
    }

    #[test]
    fn closed_connections_defaults_to_empty() {
        let snapshot = ClosedConnectionsSnapshot::default();
        assert!(snapshot.records.is_empty());
        assert_eq!(snapshot.total_closed, 0);
    }
}
