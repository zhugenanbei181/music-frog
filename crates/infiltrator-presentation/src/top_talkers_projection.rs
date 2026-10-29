//! Presentation models and projections for Top-Talkers application network ranking (FLPARITY-032).

use infiltrator_contract::app_traffic::{AppTrafficAggregate, TopTalkersReport, TopTalkersSort};
use infiltrator_domain::connection_view::{ConnectionView, process_display_name};
use infiltrator_shared::byte_units::format_bytes;
use std::collections::HashMap;

/// Visual card projection for an individual application's traffic usage.
#[derive(Clone, Debug, PartialEq)]
pub struct TopTalkerCardProjection {
    /// Display name of the application.
    pub process_name: String,
    /// Category label in Chinese (e.g. "浏览器", "通讯社交").
    pub category_zh: &'static str,
    /// Canonical icon asset token identifier.
    pub icon_name: String,
    /// Formatted active connection count label (e.g. "12 连接").
    pub connection_count_label: String,
    /// Formatted total data transfer (e.g. "245.8 MB").
    pub total_traffic_label: String,
    /// Formatted upload transfer.
    pub upload_label: String,
    /// Formatted download transfer.
    pub download_label: String,
    /// Share of overall active network bandwidth (0.0 .. 1.0).
    pub percentage_of_total: f32,
    /// IDs of all active connections owned by this application.
    pub connection_ids: Vec<String>,
}

impl TopTalkerCardProjection {
    pub fn of(agg: &AppTrafficAggregate, total_bandwidth: u64) -> Self {
        let total = agg.total_bytes();
        let percentage_of_total = if total_bandwidth > 0 {
            (total as f32 / total_bandwidth as f32).clamp(0.0, 1.0)
        } else {
            0.0
        };

        Self {
            process_name: agg.process_name.clone(),
            category_zh: agg.icon_token.category.label_zh(),
            icon_name: agg.icon_token.icon_name.clone(),
            connection_count_label: format!("{} 连接", agg.connection_count),
            total_traffic_label: format_bytes(total),
            upload_label: format_bytes(agg.upload_total_bytes),
            download_label: format_bytes(agg.download_total_bytes),
            percentage_of_total,
            connection_ids: agg.connection_ids.clone(),
        }
    }
}

/// Aggregate live connections into a typed Top-Talkers report.
pub fn aggregate_top_talkers<C: ConnectionView>(
    connections: &[C],
    sort: TopTalkersSort,
) -> TopTalkersReport {
    let mut map: HashMap<String, AppTrafficAggregate> = HashMap::new();

    for conn in connections {
        let name = process_display_name(conn.view_process_path());
        let process_key = if name.is_empty() {
            "Unknown".to_string()
        } else {
            name
        };

        let entry = map
            .entry(process_key.clone())
            .or_insert_with(|| AppTrafficAggregate::new(process_key));

        entry.add_connection(
            conn.view_id(),
            conn.view_upload_total(),
            conn.view_download_total(),
        );
    }

    let apps: Vec<AppTrafficAggregate> = map.into_values().collect();
    TopTalkersReport::of(apps, sort)
}

/// Project top talker application cards from a report.
pub fn project_top_talker_cards(report: &TopTalkersReport) -> Vec<TopTalkerCardProjection> {
    report
        .applications
        .iter()
        .map(|app| TopTalkerCardProjection::of(app, report.total_bandwidth_bytes))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockConnection {
        id: String,
        process_path: String,
        upload: u64,
        download: u64,
    }

    impl ConnectionView for MockConnection {
        fn view_id(&self) -> &str {
            &self.id
        }
        fn view_host(&self) -> &str {
            "example.com"
        }
        fn view_destination_ip(&self) -> &str {
            "1.1.1.1"
        }
        fn view_process_path(&self) -> &str {
            &self.process_path
        }
        fn view_upload_total(&self) -> u64 {
            self.upload
        }
        fn view_download_total(&self) -> u64 {
            self.download
        }
    }

    #[test]
    fn top_talkers_aggregation_and_cards() {
        let conns = vec![
            MockConnection {
                id: "c1".into(),
                process_path: "/usr/bin/firefox".into(),
                upload: 100,
                download: 900,
            },
            MockConnection {
                id: "c2".into(),
                process_path: "/usr/bin/firefox".into(),
                upload: 200,
                download: 800,
            },
            MockConnection {
                id: "c3".into(),
                process_path: "/usr/bin/curl".into(),
                upload: 50,
                download: 50,
            },
        ];

        let report = aggregate_top_talkers(&conns, TopTalkersSort::TotalBytesDesc);
        assert_eq!(report.applications.len(), 2);
        assert_eq!(report.total_connections, 3);
        assert_eq!(report.total_bandwidth_bytes, 2100);

        let cards = project_top_talker_cards(&report);
        assert_eq!(cards.len(), 2);
        assert_eq!(cards[0].process_name, "firefox");
        assert_eq!(cards[0].category_zh, "浏览器");
        assert_eq!(cards[0].icon_name, "firefox");
        assert_eq!(cards[0].connection_count_label, "2 连接");
        assert!(cards[0].percentage_of_total > 0.9);
        assert_eq!(cards[0].connection_ids.len(), 2);
    }
}
